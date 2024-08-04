use chrono::{DateTime, Utc};
use future::Future;
use futures_util::try_join;
use std::pin::Pin;
use std::sync::Arc;
use stream::FuturesUnordered;

use serenity::builder::CreateMessage;
use serenity::{
    http::CacheHttp,
    model::{
        channel::Message,
        id::{ChannelId, UserId},
    },
};

use announcer::MemberToChannels;
use youmubot_prelude::announcer::CacheAndHttp;
use youmubot_prelude::stream::TryStreamExt;
use youmubot_prelude::*;

use crate::discord::calculate_weighted_map_age;
use crate::discord::db::OsuUserMode;
use crate::{
    discord::cache::save_beatmap,
    discord::oppai_cache::BeatmapContent,
    models::{Mode, Score, User, UserEventRank},
    request::UserID,
    Client as Osu,
};

use super::db::OsuUser;
use super::interaction::score_components;
use super::{calculate_weighted_map_length, OsuEnv};
use super::{embeds::score_embed, BeatmapWithMode};

/// osu! announcer's unique announcer key.
pub const ANNOUNCER_KEY: &str = "osu";
const MAX_FAILURES: u8 = 64;

/// The announcer struct implementing youmubot_prelude::Announcer
pub struct Announcer {}

impl Announcer {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl youmubot_prelude::Announcer for Announcer {
    async fn updates(
        &mut self,
        ctx: CacheAndHttp,
        d: AppData,
        channels: MemberToChannels,
    ) -> Result<()> {
        let env = d.read().await.get::<OsuEnv>().unwrap().clone();
        // For each user...
        let users = env.saved_users.all().await?;
        users
            .into_iter()
            .map(|osu_user| {
                channels
                    .channels_of(ctx.clone(), osu_user.user_id)
                    .then(|chs| self.update_user(ctx.clone(), &env, osu_user, chs))
            })
            .collect::<stream::FuturesUnordered<_>>()
            .collect::<()>()
            .await;
        Ok(())
    }
}

impl Announcer {
    async fn update_user(
        &self,
        ctx: impl CacheHttp + Clone + 'static,
        env: &OsuEnv,
        mut user: OsuUser,
        broadcast_to: Vec<ChannelId>,
    ) {
        if broadcast_to.is_empty() {
            return; // Skip update if there are no broadcasting channels
        }
        if user.failures == MAX_FAILURES {
            return;
        }
        const MODES: [Mode; 4] = [Mode::Std, Mode::Taiko, Mode::Catch, Mode::Mania];
        let now = chrono::Utc::now();
        let broadcast_to = Arc::new(broadcast_to);
        let mut to_announce = Vec::<Pin<Box<dyn Future<Output = ()> + Send>>>::new();
        for mode in MODES {
            let (u, top, events) = match self.fetch_user_data(env, now, &user, mode).await {
                Ok(v) => v,
                Err(err) => {
                    eprintln!(
                        "[osu] Updating `{}`[{}] failed with: {}",
                        user.username, user.id, err
                    );
                    user.failures += 1;
                    if user.failures == MAX_FAILURES {
                        eprintln!(
                            "[osu] Too many failures, disabling: `{}`[{}]",
                            user.username, user.id
                        );
                    }
                    break;
                }
            };
            // update stats
            let stats = OsuUserMode {
                pp: u.pp.unwrap_or(0.0),
                map_length: calculate_weighted_map_length(&top, &env.beatmaps, mode)
                    .await
                    .pls_ok()
                    .unwrap_or(0.0),
                map_age: calculate_weighted_map_age(&top, &env.beatmaps, mode)
                    .await
                    .pls_ok()
                    .unwrap_or(0),
                last_update: now,
            };
            let last = user.modes.insert(mode, stats);

            // broadcast
            let mention = user.user_id;
            let broadcast_to = broadcast_to.clone();
            let ctx = ctx.clone();
            let env = env.clone();
            if let Some(last) = last {
                to_announce.push(Box::pin(async move {
                    let top = top
                        .into_iter()
                        .enumerate()
                        .filter(|(_, s)| Self::is_announceable_date(s.date, last.last_update, now))
                        .map(|(rank, score)| {
                            CollectedScore::from_top_score(&u, score, mode, rank as u8 + 1)
                        });
                    let recents = events
                        .into_iter()
                        .map(|e| CollectedScore::from_event(&env.client, &u, e))
                        .collect::<FuturesUnordered<_>>()
                        .filter_map(|v| future::ready(v.pls_ok()))
                        .collect::<Vec<_>>()
                        .await
                        .into_iter();
                    top.chain(recents)
                        .map(|v| v.send_message(&ctx, &env, mention, &broadcast_to))
                        .collect::<FuturesUnordered<_>>()
                        .filter_map(|v| future::ready(v.pls_ok().map(|_| ())))
                        .collect::<()>()
                        .await
                }));
            }
        }
        user.failures = 0;
        let user_id = user.user_id;
        if let Some(true) = env.saved_users.save(user).await.pls_ok() {
            to_announce.into_iter().for_each(|v| {
                spawn_future(v);
            });
        } else {
            eprintln!("[osu] Skipping user {} due to raced update", user_id)
        }
    }

    fn is_announceable_date(
        s: DateTime<Utc>,
        last_update: impl Into<Option<DateTime<Utc>>>,
        now: DateTime<Utc>,
    ) -> bool {
        (match last_update.into() {
            Some(lu) => s > lu,
            None => true,
        }) && s <= now
    }

    /// Handles an user/mode scan, announces all possible new scores, return the new pp value.
    async fn fetch_user_data(
        &self,
        env: &OsuEnv,
        now: chrono::DateTime<chrono::Utc>,
        osu_user: &OsuUser,
        mode: Mode,
    ) -> Result<(User, Vec<Score>, Vec<UserEventRank>), Error> {
        let stats = osu_user.modes.get(&mode).cloned();
        let last_update = stats.as_ref().map(|v| v.last_update);
        let user_id = UserID::ID(osu_user.id);
        let user = {
            let days_since_last_update = stats
                .as_ref()
                .map(|v| (now - v.last_update).num_days() + 1)
                .unwrap_or(30);
            env.client.user(&user_id, move |f| {
                f.mode(mode)
                    .event_days(days_since_last_update.min(31) as u8)
            })
        };
        let top_scores = env
            .client
            .user_best(user_id.clone(), |f| f.mode(mode).limit(100));
        let (user, top_scores) = try_join!(user, top_scores)?;
        let mut user = user.unwrap();
        // if top scores exist, user would too
        let events = std::mem::replace(&mut user.events, vec![])
            .into_iter()
            .filter_map(|v| v.to_event_rank())
            .filter(|s| Self::is_announceable_date(s.date, last_update, now))
            .collect::<Vec<_>>();
        Ok((user, top_scores, events))
    }
}

struct CollectedScore<'a> {
    pub user: &'a User,
    pub score: Score,
    pub mode: Mode,
    pub kind: ScoreType,
}

impl<'a> CollectedScore<'a> {
    fn from_top_score(user: &'a User, score: Score, mode: Mode, rank: u8) -> Self {
        Self {
            user,
            score,
            mode,
            kind: ScoreType::TopRecord(rank),
        }
    }

    async fn from_event(
        osu: &Osu,
        user: &'a User,
        event: UserEventRank,
    ) -> Result<CollectedScore<'a>> {
        let scores = osu
            .scores(event.beatmap_id, |f| {
                f.user(UserID::ID(user.id)).mode(event.mode)
            })
            .await?;
        let score = match scores
            .into_iter()
            .find(|s| (s.date - event.date).abs() < chrono::TimeDelta::seconds(5))
        {
            Some(v) => v,
            None => {
                return Err(Error::msg(format!(
                    "cannot get score for map..., event = {:?}",
                    event
                )))
            }
        };
        Ok(Self {
            user,
            score,
            mode: event.mode,
            kind: ScoreType::WorldRecord(event.rank),
        })
    }
}

impl<'a> CollectedScore<'a> {
    async fn send_message(
        self,
        ctx: impl CacheHttp,
        env: &OsuEnv,
        mention: UserId,
        channels: &[ChannelId],
    ) -> Result<Vec<Message>> {
        let (bm, content) = self.get_beatmap(env).await?;
        channels
            .iter()
            .map(|c| self.send_message_to(mention, *c, &ctx, env, &bm, &content))
            .collect::<stream::FuturesUnordered<_>>()
            .try_collect()
            .await
    }

    async fn get_beatmap(&self, env: &OsuEnv) -> Result<(BeatmapWithMode, BeatmapContent)> {
        let beatmap = env
            .beatmaps
            .get_beatmap_default(self.score.beatmap_id)
            .await?;
        let content = env.oppai.get_beatmap(beatmap.beatmap_id).await?;
        Ok((BeatmapWithMode(beatmap, self.mode), content))
    }

    async fn send_message_to(
        &self,
        mention: UserId,
        channel: ChannelId,
        ctx: impl CacheHttp,
        env: &OsuEnv,
        bm: &BeatmapWithMode,
        content: &BeatmapContent,
    ) -> Result<Message> {
        let guild = match channel.to_channel(&ctx).await?.guild() {
            Some(gc) => gc.guild_id,
            None => {
                eprintln!("Not a guild channel: {}", channel);
                return Err(Error::msg("Trying to announce to a non-server channel"));
            }
        };

        let member = match guild.member(&ctx, mention).await {
            Ok(mem) => mem,
            Err(e) => {
                eprintln!("Cannot get member {}: {}", mention, e);
                return Err(e.into());
            }
        };
        let m = channel
            .send_message(
                &ctx,
                CreateMessage::new()
                    .content(match self.kind {
                        ScoreType::TopRecord(rank) => {
                            if rank <= 25 {
                                format!("New leaderboard record from {}!", mention.mention())
                            } else {
                                format!("New leaderboard record from **{}**!", member.distinct())
                            }
                        }
                        ScoreType::WorldRecord(rank) => {
                            if (self.mode == Mode::Std && rank <= 100)
                                || (self.mode != Mode::Std && rank <= 50)
                            {
                                format!("New leaderboard record from {}!", mention.mention())
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
                    })
                    .components(vec![score_components(Some(guild))]),
            )
            .await?;

        save_beatmap(&env, channel, bm).await.pls_ok();
        Ok(m)
    }
}

enum ScoreType {
    TopRecord(u8),
    WorldRecord(u16),
}
