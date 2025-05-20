use chrono::{DateTime, Utc};
use futures_util::try_join;
use serenity::all::Member;
use std::collections::HashMap;
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
use youmubot_prelude::*;

use crate::discord::calculate_weighted_map_age;
use crate::discord::db::OsuUserMode;
use crate::models::UserHeader;
use crate::scores::LazyBuffer;
use crate::{
    discord::cache::save_beatmap,
    discord::oppai_cache::BeatmapContent,
    models::{Mode, Score, User, UserEventRank},
    request::UserID,
    OsuClient as Osu,
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
        let last_update = user
            .modes
            .iter()
            .map(|(k, v)| (*k, v.last_update))
            .collect::<HashMap<_, _>>();
        let now = chrono::Utc::now();
        let broadcast_to = Arc::new(broadcast_to);
        let mut to_announce = Vec::<CollectedScore>::new();
        for mode in MODES {
            let (u, top) = match self.fetch_user_data(env, &user, mode).await {
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
            if u.username != user.username {
                user.username = u.username.clone().into();
            }
            user.preferred_mode = u.preferred_mode;
            let last = user.modes.insert(mode, stats);

            // broadcast
            if let Some(last) = last {
                to_announce.extend(
                    top.into_iter()
                        .enumerate()
                        .filter(|(_, s)| Self::is_announceable_date(s.date, last.last_update, now))
                        .map(|(rank, score)| {
                            CollectedScore::from_top_score(&u, score, mode, rank as u8 + 1)
                        }),
                );
            }
        }
        if let Some(recents) = env
            .client
            .user_events(UserID::ID(user.id))
            .and_then(|v| v.get_all())
            .await
            .pls_ok()
        {
            if let Some(header) = env.client.user_header(user.id).await.pls_ok().flatten() {
                let recents = recents
                    .into_iter()
                    .filter_map(|v| v.to_event_rank())
                    .filter(|s| Self::is_worth_announcing(s))
                    .filter(|s| {
                        let lu = last_update.get(&s.mode).cloned();
                        let f = Self::is_announceable_date(s.date, lu, now);
                        f
                    })
                    .map(|e| CollectedScore::from_event(&env.client, header.clone(), e))
                    .collect::<FuturesUnordered<_>>()
                    .filter_map(|v| future::ready(v.pls_ok()))
                    .collect::<Vec<_>>()
                    .await;
                to_announce =
                    CollectedScore::merge(to_announce.into_iter().chain(recents)).collect();
            }
        }

        user.failures = 0;
        let user_id = user.user_id;
        if let Some(true) = env.saved_users.save(user).await.pls_ok() {
            let env = env.clone();
            let ctx = ctx.clone();
            let broadcast_to = broadcast_to.clone();
            spawn_future(async move {
                for v in to_announce.into_iter() {
                    v.send_message(&ctx, &env, user_id, &broadcast_to)
                        .await
                        .pls_ok();
                }
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

    // Is an user_event worth announcing?
    fn is_worth_announcing(s: &UserEventRank) -> bool {
        if s.mode != Mode::Std && s.rank > 50 {
            return false;
        }
        true
    }

    /// Handles an user/mode scan, announces all possible new scores, return the new pp value.
    async fn fetch_user_data(
        &self,
        env: &OsuEnv,
        osu_user: &OsuUser,
        mode: Mode,
    ) -> Result<(User, Vec<Score>), Error> {
        let user_id = UserID::ID(osu_user.id);
        let user = env.client.user(&user_id, move |f| f.mode(mode));
        let top_scores = env
            .client
            .user_best(user_id.clone(), move |f| f.mode(mode))
            .and_then(|v| v.get_all());
        let (user, top_scores) = try_join!(user, top_scores)?;
        let Some(user) = user else {
            return Err(error!("user not found"));
        };
        Ok((user, top_scores))
    }
}

struct CollectedScore {
    pub user: UserHeader,
    pub score: Score,
    pub mode: Mode,
    pub kind: ScoreType,
}

impl CollectedScore {
    fn merge(scores: impl IntoIterator<Item = Self>) -> impl Iterator<Item = Self> {
        let mut mp = std::collections::HashMap::<u64, Self>::new();
        scores
            .into_iter()
            .filter_map(|s| s.score.id.map(|id| (id, s)))
            .for_each(|(id, s)| {
                mp.entry(id)
                    .and_modify(|v| {
                        v.kind = v.kind.merge(s.kind);
                    })
                    .or_insert(s);
            });
        mp.into_values()
    }

    fn from_top_score(user: impl Into<UserHeader>, score: Score, mode: Mode, rank: u8) -> Self {
        Self {
            user: user.into(),
            score,
            mode,
            kind: ScoreType::top(rank),
        }
    }

    async fn from_event(
        osu: &Osu,
        user: impl Into<UserHeader>,
        event: UserEventRank,
    ) -> Result<CollectedScore> {
        let user = user.into();
        let user_id = user.id;
        let mut scores = osu
            .scores(event.beatmap_id, |f| {
                f.user(UserID::ID(user_id)).mode(event.mode)
            })
            .await?;
        let score = match scores
            .find(|s| (s.date - event.date).abs() < chrono::TimeDelta::seconds(5))
            .await?
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
            score: score.clone(),
            mode: event.mode,
            kind: ScoreType::world(event.rank),
        })
    }
}

impl CollectedScore {
    async fn send_message(
        self,
        ctx: impl CacheHttp,
        env: &OsuEnv,
        mention: UserId,
        channels: &[ChannelId],
    ) -> Result<Vec<Message>> {
        let (bm, content) = self.get_beatmap(env).await?;
        Ok(channels
            .iter()
            .map(|c| self.send_message_to(mention, *c, &ctx, env, &bm, &content))
            .collect::<stream::FuturesUnordered<_>>()
            .filter_map(|v| future::ready(v.pls_ok()))
            .collect::<Vec<_>>()
            .await)
    }

    async fn get_beatmap(&self, env: &OsuEnv) -> Result<(BeatmapWithMode, BeatmapContent)> {
        let beatmap = env
            .beatmaps
            .get_beatmap_default(self.score.beatmap_id)
            .await?;
        let content = env.oppai.get_beatmap(beatmap.beatmap_id).await?;
        Ok((BeatmapWithMode(beatmap, Some(self.mode)), content))
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
                    .content(self.kind.announcement_msg(self.mode, &member))
                    .embed({
                        let b = score_embed(&self.score, bm, content, self.user.clone());
                        let b = if let Some(rank) = self.kind.top_record {
                            b.top_record(rank)
                        } else {
                            b
                        };
                        let b = if let Some(rank) = self.kind.world_record {
                            b.world_record(rank)
                        } else {
                            b
                        };
                        b.build()
                    })
                    .components(vec![score_components(Some(guild))]),
            )
            .await?;

        save_beatmap(env, channel, bm).await.pls_ok();
        Ok(m)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ScoreType {
    pub top_record: Option<u8>,
    pub world_record: Option<u16>,
}

impl ScoreType {
    fn top(rank: u8) -> Self {
        Self {
            top_record: Some(rank),
            world_record: None,
        }
    }
    fn world(rank: u16) -> Self {
        Self {
            top_record: None,
            world_record: Some(rank),
        }
    }

    fn merge(self, other: Self) -> Self {
        Self {
            top_record: self.top_record.or(other.top_record),
            world_record: self.world_record.or(other.world_record),
        }
    }

    fn announcement_msg(&self, mode: Mode, mention: &Member) -> String {
        let mention_user = self.top_record.is_some_and(|r| r <= 25)
            || self
                .world_record
                .is_some_and(|w| if mode == Mode::Std { w <= 100 } else { w <= 50 });
        let title = if self.top_record.is_some() {
            "New top record"
        } else if self.world_record.is_some() {
            "New leaderboard record"
        } else {
            "New record"
        };
        if mention_user {
            format!("{} from {}!", title, mention.mention())
        } else {
            format!("{} from **{}**!", title, mention.distinct())
        }
    }
}
