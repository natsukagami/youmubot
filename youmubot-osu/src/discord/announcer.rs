use super::db::{OsuSavedUsers, OsuUser};
use super::OsuClient;
use super::{embeds::score_embed, BeatmapWithMode};
use crate::{
    discord::beatmap_cache::BeatmapMetaCache,
    discord::cache::save_beatmap,
    discord::oppai_cache::{BeatmapCache, BeatmapContent},
    models::{Mode, Score, User, UserEventRank},
    request::UserID,
    Client as Osu,
};
use announcer::MemberToChannels;
use serenity::builder::CreateMessage;
use serenity::{
    http::CacheHttp,
    model::{
        channel::Message,
        id::{ChannelId, UserId},
    },
};
use std::{convert::TryInto, sync::Arc};
use youmubot_prelude::announcer::CacheAndHttp;
use youmubot_prelude::stream::{FuturesUnordered, TryStreamExt};
use youmubot_prelude::*;

/// osu! announcer's unique announcer key.
pub const ANNOUNCER_KEY: &str = "osu";

/// The announcer struct implementing youmubot_prelude::Announcer
pub struct Announcer {
    client: Arc<Osu>,
}

impl Announcer {
    pub fn new(client: Arc<Osu>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl youmubot_prelude::Announcer for Announcer {
    async fn updates(
        &mut self,
        c: CacheAndHttp,
        d: AppData,
        channels: MemberToChannels,
    ) -> Result<()> {
        // For each user...
        let users = {
            let data = d.read().await;
            let data = data.get::<OsuSavedUsers>().unwrap();
            data.all().await?
        };
        let now = chrono::Utc::now();
        users
            .into_iter()
            .map(|mut osu_user| {
                let user_id = osu_user.user_id;
                let channels = &channels;
                let ctx = Context {
                    c: c.clone(),
                    data: d.clone(),
                };
                let s = &*self;
                async move {
                    let channels = channels.channels_of(ctx.c.clone(), user_id).await;
                    if channels.is_empty() {
                        return; // We don't wanna update an user without any active server
                    }
                    match [Mode::Std, Mode::Taiko, Mode::Catch, Mode::Mania]
                        .into_iter()
                        .map(|m| {
                            s.handle_user_mode(&ctx, now, &osu_user, user_id, channels.clone(), m)
                        })
                        .collect::<stream::FuturesOrdered<_>>()
                        .try_collect::<Vec<_>>()
                        .await
                    {
                        Ok(v) => {
                            osu_user.pp = v
                                .iter()
                                .map(|u| u.pp)
                                .collect::<Vec<_>>()
                                .try_into()
                                .unwrap();
                            osu_user.username = v.into_iter().next().unwrap().username.into();
                            osu_user.last_update = now;
                            osu_user.std_weighted_map_length =
                                Self::std_weighted_map_length(&ctx, &osu_user)
                                    .await
                                    .pls_ok();
                            let id = osu_user.id;
                            println!("{:?}", osu_user);
                            ctx.data
                                .read()
                                .await
                                .get::<OsuSavedUsers>()
                                .unwrap()
                                .save(osu_user)
                                .await
                                .pls_ok();
                            println!("updating {} done", id);
                        }
                        Err(e) => {
                            eprintln!("osu: Cannot update {}: {}", osu_user.id, e);
                        }
                    };
                }
            })
            .collect::<stream::FuturesUnordered<_>>()
            .collect::<()>()
            .await;
        Ok(())
    }
}

impl Announcer {
    /// Handles an user/mode scan, announces all possible new scores, return the new pp value.
    async fn handle_user_mode(
        &self,
        ctx: &Context,
        now: chrono::DateTime<chrono::Utc>,
        osu_user: &OsuUser,
        user_id: UserId,
        channels: Vec<ChannelId>,
        mode: Mode,
    ) -> Result<User, Error> {
        let days_since_last_update = (now - osu_user.last_update).num_days() + 1;
        let last_update = osu_user.last_update;
        let (scores, user) = {
            let scores = self.scan_user(osu_user, mode).await?;
            let user = self
                .client
                .user(UserID::ID(osu_user.id), |f| {
                    f.mode(mode)
                        .event_days(days_since_last_update.min(31) as u8)
                })
                .await?
                .ok_or_else(|| Error::msg("user not found"))?;
            (scores, user)
        };
        let client = self.client.clone();
        let ctx = ctx.clone();
        let _user = user.clone();
        spawn_future(async move {
            let event_scores = user
                .events
                .iter()
                .filter_map(|u| u.to_event_rank())
                .filter(|u| u.mode == mode && u.date > last_update && u.date <= now)
                .map(|ev| CollectedScore::from_event(&client, &user, ev, user_id, &channels[..]))
                .collect::<stream::FuturesUnordered<_>>()
                .filter_map(|u| future::ready(u.pls_ok()))
                .collect::<Vec<_>>()
                .await;
            let top_scores = scores.into_iter().filter_map(|(rank, score)| {
                if score.date > last_update && score.date <= now {
                    Some(CollectedScore::from_top_score(
                        &user,
                        score,
                        mode,
                        rank,
                        user_id,
                        &channels[..],
                    ))
                } else {
                    None
                }
            });
            event_scores
                .into_iter()
                .chain(top_scores)
                .map(|v| v.send_message(&ctx))
                .collect::<stream::FuturesUnordered<_>>()
                .try_collect::<Vec<_>>()
                .await
                .pls_ok();
        });
        Ok(_user)
    }

    async fn scan_user(&self, u: &OsuUser, mode: Mode) -> Result<Vec<(u8, Score)>, Error> {
        let scores = self
            .client
            .user_best(UserID::ID(u.id), |f| f.mode(mode).limit(25))
            .await?;
        let scores = scores
            .into_iter()
            .enumerate()
            .filter(|(_, s)| s.date >= u.last_update)
            .map(|(i, v)| ((i + 1) as u8, v))
            .collect();
        Ok(scores)
    }

    async fn std_weighted_map_length(ctx: &Context, u: &OsuUser) -> Result<f64> {
        let data = ctx.data.read().await;
        let client = data.get::<OsuClient>().unwrap().clone();
        let cache = data.get::<BeatmapMetaCache>().unwrap();
        let scores = client
            .user_best(UserID::ID(u.id), |f| f.mode(Mode::Std).limit(100))
            .await?;
        scores
            .into_iter()
            .enumerate()
            .map(|(i, s)| async move {
                let beatmap = cache.get_beatmap_default(s.beatmap_id).await?;
                const SCALING_FACTOR: f64 = 0.975;
                Ok(beatmap
                    .difficulty
                    .apply_mods(s.mods, 0.0 /* dont care */)
                    .drain_length
                    .as_secs_f64()
                    * (SCALING_FACTOR.powi(i as i32)))
            })
            .collect::<FuturesUnordered<_>>()
            .try_fold(0.0, |a, b| future::ready(Ok(a + b)))
            .await
    }
}

#[derive(Clone)]
struct Context {
    data: AppData,
    c: CacheAndHttp,
}

struct CollectedScore<'a> {
    pub user: &'a User,
    pub score: Score,
    pub mode: Mode,
    pub kind: ScoreType,

    pub discord_user: UserId,
    pub channels: &'a [ChannelId],
}

impl<'a> CollectedScore<'a> {
    fn from_top_score(
        user: &'a User,
        score: Score,
        mode: Mode,
        rank: u8,
        discord_user: UserId,
        channels: &'a [ChannelId],
    ) -> Self {
        Self {
            user,
            score,
            mode,
            kind: ScoreType::TopRecord(rank),
            discord_user,
            channels,
        }
    }

    async fn from_event(
        osu: &Osu,
        user: &'a User,
        event: UserEventRank,
        discord_user: UserId,
        channels: &'a [ChannelId],
    ) -> Result<CollectedScore<'a>> {
        let scores = osu
            .scores(event.beatmap_id, |f| {
                f.user(UserID::ID(user.id)).mode(event.mode)
            })
            .await?;
        let score = match scores.into_iter().next() {
            Some(v) => v,
            None => return Err(Error::msg("cannot get score for map...")),
        };
        Ok(Self {
            user,
            score,
            mode: event.mode,
            kind: ScoreType::WorldRecord(event.rank),
            discord_user,
            channels,
        })
    }
}

impl<'a> CollectedScore<'a> {
    async fn send_message(self, ctx: &Context) -> Result<Vec<Message>> {
        let (bm, content) = self.get_beatmap(ctx).await?;
        self.channels
            .iter()
            .map(|c| self.send_message_to(*c, ctx, &bm, &content))
            .collect::<stream::FuturesUnordered<_>>()
            .try_collect()
            .await
    }

    async fn get_beatmap(&self, ctx: &Context) -> Result<(BeatmapWithMode, BeatmapContent)> {
        let data = ctx.data.read().await;
        let cache = data.get::<BeatmapMetaCache>().unwrap();
        let oppai = data.get::<BeatmapCache>().unwrap();
        let beatmap = cache.get_beatmap_default(self.score.beatmap_id).await?;
        let content = oppai.get_beatmap(beatmap.beatmap_id).await?;
        Ok((BeatmapWithMode(beatmap, self.mode), content))
    }

    async fn send_message_to(
        &self,
        channel: ChannelId,
        ctx: &Context,
        bm: &BeatmapWithMode,
        content: &BeatmapContent,
    ) -> Result<Message> {
        let guild = match channel.to_channel(&ctx.c).await?.guild() {
            Some(gc) => gc.guild_id,
            None => {
                eprintln!("Not a guild channel: {}", channel);
                return Err(Error::msg("Trying to announce to a non-server channel"));
            }
        };

        let member = match guild.member(&ctx.c, self.discord_user).await {
            Ok(mem) => mem,
            Err(e) => {
                eprintln!("Cannot get member {}: {}", self.discord_user, e);
                return Err(e.into());
            }
        };
        let m = channel
            .send_message(
                ctx.c.http(),
                CreateMessage::new()
                    .content(match self.kind {
                        ScoreType::TopRecord(_) => {
                            format!("New top record from {}!", self.discord_user.mention())
                        }
                        ScoreType::WorldRecord(rank) => {
                            if rank <= 100 {
                                format!(
                                    "New leaderboard record from {}!",
                                    self.discord_user.mention()
                                )
                            } else {
                                format!("New leaderboard record from **{}**!", member.distinct())
                            }
                        }
                    })
                    .embed({
                        let mut b = score_embed(&self.score, bm, content, self.user);
                        match self.kind {
                            ScoreType::TopRecord(rank) => b.top_record(rank),
                            ScoreType::WorldRecord(rank) => b.world_record(rank),
                        }
                        .build()
                    }),
            )
            .await?;
        save_beatmap(
            ctx.data.read().await.get::<crate::discord::Env>().unwrap(),
            channel,
            bm,
        )
        .await
        .pls_ok();
        Ok(m)
    }
}

enum ScoreType {
    TopRecord(u8),
    WorldRecord(u16),
}
