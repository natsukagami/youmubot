use super::db::{OsuSavedUsers, OsuUser};
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
use serenity::{
    http::CacheHttp,
    model::{
        channel::Message,
        id::{ChannelId, UserId},
    },
    CacheAndHttp,
};
use std::{convert::TryInto, sync::Arc};
use youmubot_prelude::*;

/// osu! announcer's unique announcer key.
pub const ANNOUNCER_KEY: &str = "osu";

/// The announcer struct implementing youmubot_prelude::Announcer
pub struct Announcer {
    client: Arc<Osu>,
}

impl Announcer {
    pub fn new(client: Osu) -> Self {
        Self {
            client: Arc::new(client),
        }
    }
}

#[async_trait]
impl youmubot_prelude::Announcer for Announcer {
    async fn updates(
        &mut self,
        c: Arc<CacheAndHttp>,
        d: AppData,
        channels: MemberToChannels,
    ) -> Result<()> {
        // For each user...
        let data = d.read().await;
        let data = data.get::<OsuSavedUsers>().unwrap();
        let now = chrono::Utc::now();
        let users = data.all().await?;
        users
            .into_iter()
            .map(|mut osu_user| {
                let user_id = osu_user.user_id;
                let channels = &channels;
                let ctx = Context {
                    c: c.clone(),
                    data: d.clone(),
                };
                let s = &self;
                async move {
                    let channels = channels.channels_of(ctx.c.clone(), user_id).await;
                    if channels.is_empty() {
                        return; // We don't wanna update an user without any active server
                    }
                    match std::array::IntoIter::new([
                        Mode::Std,
                        Mode::Taiko,
                        Mode::Catch,
                        Mode::Mania,
                    ])
                    .map(|m| s.handle_user_mode(&ctx, now, &osu_user, user_id, channels.clone(), m))
                    .collect::<stream::FuturesOrdered<_>>()
                    .try_collect::<Vec<_>>()
                    .await
                    {
                        Ok(v) => {
                            osu_user.last_update = now;
                            osu_user.pp = v.try_into().unwrap();
                            data.save(osu_user).await.pls_ok();
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
    ) -> Result<Option<f32>, Error> {
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
        let pp = user.pp;
        let ctx = ctx.clone();
        spawn_future(async move {
            let event_scores = user
                .events
                .iter()
                .filter_map(|u| u.to_event_rank())
                .filter(|u| u.mode == mode && u.date > last_update && u.date <= now)
                .map(|ev| CollectedScore::from_event(&*client, &user, ev, user_id, &channels[..]))
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
        Ok(pp.map(|v| v as f32))
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
}

#[derive(Clone)]
struct Context {
    data: AppData,
    c: Arc<CacheAndHttp>,
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
        let (bm, content) = self.get_beatmap(&ctx).await?;
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
        let m = channel
            .send_message(ctx.c.http(), |c| {
                c.content(match self.kind {
                    ScoreType::TopRecord(_) => {
                        format!("New top record from {}!", self.discord_user.mention())
                    }
                    ScoreType::WorldRecord(_) => {
                        format!("New best score from {}!", self.discord_user.mention())
                    }
                })
                .embed(|e| {
                    let mut b = score_embed(&self.score, &bm, content, self.user);
                    match self.kind {
                        ScoreType::TopRecord(rank) => b.top_record(rank),
                        ScoreType::WorldRecord(rank) => b.world_record(rank),
                    }
                    .build(e)
                })
            })
            .await?;
        save_beatmap(&*ctx.data.read().await, channel, &bm)
            .await
            .pls_ok();
        Ok(m)
    }
}

enum ScoreType {
    TopRecord(u8),
    WorldRecord(u16),
}
