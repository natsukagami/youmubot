use core::fmt;

use crate::models::{Mode, Mods};
use crate::OsuClient;
use rosu_v2::error::OsuError;
use youmubot_prelude::*;

/// Maximum number of scores returned by the osu! api.
pub const SCORE_COUNT_LIMIT: usize = 200;

#[derive(Clone, Debug)]
pub enum UserID {
    Username(String),
    ID(u64),
}

impl fmt::Display for UserID {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UserID::Username(u) => u.fmt(f),
            UserID::ID(id) => id.fmt(f),
        }
    }
}

impl From<UserID> for rosu_v2::prelude::UserId {
    fn from(value: UserID) -> Self {
        match value {
            UserID::Username(s) => rosu_v2::request::UserId::Name(s.into()),
            UserID::ID(id) => rosu_v2::request::UserId::Id(id as u32),
        }
    }
}

impl UserID {
    pub fn from_string(s: impl Into<String>) -> UserID {
        let s = s.into();
        match s.parse::<u64>() {
            Ok(id) => UserID::ID(id),
            Err(_) => UserID::Username(s),
        }
    }
}

pub enum BeatmapRequestKind {
    Beatmap(u64),
    Beatmapset(u64),
    BeatmapHash(String),
}

fn handle_not_found<T>(v: Result<T, OsuError>) -> Result<Option<T>, OsuError> {
    match v {
        Ok(v) => Ok(Some(v)),
        Err(OsuError::NotFound) => Ok(None),
        Err(e) => Err(e),
    }
}

pub mod builders {
    use futures_util::TryStream;
    use rosu_v2::model::mods::GameModsIntermode;

    use crate::models;

    use super::OsuClient;
    use super::*;
    /// A builder for a Beatmap request.
    pub struct BeatmapRequestBuilder {
        kind: BeatmapRequestKind,
        mode: Option<(Mode, /* Converted */ bool)>,
    }
    impl BeatmapRequestBuilder {
        pub(crate) fn new(kind: BeatmapRequestKind) -> Self {
            BeatmapRequestBuilder { kind, mode: None }
        }

        pub fn maybe_mode(&mut self, mode: Option<Mode>) -> &mut Self {
            if let Some(m) = mode {
                self.mode(m, true)
            } else {
                self
            }
        }

        pub fn mode(&mut self, mode: Mode, converted: bool) -> &mut Self {
            self.mode = Some((mode, converted));
            self
        }

        pub(crate) async fn build(self, client: &OsuClient) -> Result<Vec<models::Beatmap>> {
            Ok(match self.kind {
                BeatmapRequestKind::Beatmap(id) => {
                    match handle_not_found(client.rosu.beatmap().map_id(id as u32).await)? {
                        Some(mut bm) => {
                            let set = bm.mapset.take().unwrap();
                            vec![models::Beatmap::from_rosu(bm, &set)]
                        }
                        None => vec![],
                    }
                }
                BeatmapRequestKind::Beatmapset(id) => {
                    let mut set = match handle_not_found(client.rosu.beatmapset(id as u32).await)? {
                        Some(v) => v,
                        None => return Ok(vec![]),
                    };
                    let bms = set.maps.take().unwrap();
                    bms.into_iter()
                        .map(|bm| models::Beatmap::from_rosu(bm, &set))
                        .collect()
                }
                BeatmapRequestKind::BeatmapHash(hash) => {
                    let mut bm = match handle_not_found(client.rosu.beatmap().checksum(hash).await)?
                    {
                        Some(v) => v,
                        None => return Ok(vec![]),
                    };
                    let set = bm.mapset.take().unwrap();
                    vec![models::Beatmap::from_rosu(bm, &set)]
                }
            })
        }
    }

    pub struct UserRequestBuilder {
        user: UserID,
        mode: Option<Mode>,
        event_days: Option<u8>,
    }

    impl UserRequestBuilder {
        pub(crate) fn new(user: UserID) -> Self {
            UserRequestBuilder {
                user,
                mode: None,
                event_days: None,
            }
        }

        pub fn mode(&mut self, mode: impl Into<Option<Mode>>) -> &mut Self {
            self.mode = mode.into();
            self
        }

        pub fn event_days(&mut self, event_days: u8) -> &mut Self {
            self.event_days = Some(event_days).filter(|&v| v <= 31).or(self.event_days);
            self
        }

        pub(crate) async fn build(self, client: &OsuClient) -> Result<Option<models::User>> {
            let mut r = client.rosu.user(self.user);
            if let Some(mode) = self.mode {
                r = r.mode(mode.into());
            }
            let mut user = match handle_not_found(r.await)? {
                Some(v) => v,
                None => return Ok(None),
            };
            let now = time::OffsetDateTime::now_utc()
                - time::Duration::DAY * self.event_days.unwrap_or(31);
            let mut events = handle_not_found(client.rosu.recent_activity(user.user_id).await)?
                .unwrap_or(vec![]);
            events.retain(|e: &rosu_v2::model::event::Event| (now <= e.created_at));
            let stats = user.statistics.take().unwrap();
            Ok(Some(models::User::from_rosu(user, stats, events)))
        }
    }

    pub struct ScoreRequestBuilder {
        beatmap_id: u64,
        user: Option<UserID>,
        mode: Option<Mode>,
        mods: Option<Mods>,
        limit: Option<u8>,
    }

    impl ScoreRequestBuilder {
        pub(crate) fn new(beatmap_id: u64) -> Self {
            ScoreRequestBuilder {
                beatmap_id,
                user: None,
                mode: None,
                mods: None,
                limit: None,
            }
        }

        pub fn user(&mut self, u: UserID) -> &mut Self {
            self.user = Some(u);
            self
        }

        pub fn mode(&mut self, mode: impl Into<Option<Mode>>) -> &mut Self {
            self.mode = mode.into();
            self
        }

        pub fn mods(&mut self, mods: Mods) -> &mut Self {
            self.mods = Some(mods);
            self
        }

        pub fn limit(&mut self, limit: u8) -> &mut Self {
            self.limit = Some(limit)
                .filter(|&v| v <= SCORE_COUNT_LIMIT as u8)
                .or(self.limit);
            self
        }

        pub(crate) async fn build(self, osu: &OsuClient) -> Result<Vec<models::Score>> {
            let scores = handle_not_found(match self.user {
                Some(user) => {
                    let mut r = osu.rosu.beatmap_user_scores(self.beatmap_id as u32, user);
                    if let Some(mode) = self.mode {
                        r = r.mode(mode.into());
                    }
                    match self.mods {
                        Some(mods) => r.await.map(|mut ss| {
                            // let mods = GameModsIntermode::from(mods.inner);
                            ss.retain(|s| {
                                Mods::from_gamemods(s.mods.clone(), s.set_on_lazer).contains(&mods)
                            });
                            ss
                        }),
                        None => r.await,
                    }
                }
                None => {
                    let mut r = osu.rosu.beatmap_scores(self.beatmap_id as u32).global();
                    if let Some(mode) = self.mode {
                        r = r.mode(mode.into());
                    }
                    if let Some(mods) = self.mods {
                        r = r.mods(GameModsIntermode::from(mods.inner));
                    }
                    if let Some(limit) = self.limit {
                        r = r.limit(limit as u32);
                    }
                    r.await
                }
            })?
            .ok_or_else(|| error!("beatmap or user not found"))?;
            Ok(scores.into_iter().map(|v| v.into()).collect())
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) enum UserScoreType {
        Recent,
        Best,
        Pin,
    }

    #[derive(Debug, Clone)]
    pub struct UserScoreRequestBuilder {
        score_type: UserScoreType,
        user: UserID,
        mode: Option<Mode>,
        limit: Option<usize>,
        include_fails: bool,
    }

    impl UserScoreRequestBuilder {
        pub(crate) fn new(score_type: UserScoreType, user: UserID) -> Self {
            UserScoreRequestBuilder {
                score_type,
                user,
                mode: None,
                limit: None,
                include_fails: true,
            }
        }

        pub fn mode(&mut self, m: Mode) -> &mut Self {
            self.mode = Some(m);
            self
        }

        pub fn limit(&mut self, limit: usize) -> &mut Self {
            self.limit = if limit > SCORE_COUNT_LIMIT {
                self.limit
            } else {
                Some(limit)
            };
            self
        }

        pub fn include_fails(&mut self, include_fails: bool) -> &mut Self {
            self.include_fails = include_fails;
            self
        }

        async fn with_offset(
            self,
            offset: Option<usize>,
            client: OsuClient,
        ) -> Result<Option<(Vec<models::Score>, Option<usize>)>> {
            const MAXIMUM_LIMIT: usize = 100;
            let offset = if let Some(offset) = offset {
                offset
            } else {
                return Ok(None);
            };
            let count = match self.limit {
                Some(limit) => (limit - offset).min(MAXIMUM_LIMIT),
                None => MAXIMUM_LIMIT,
            };
            if count == 0 {
                return Ok(None);
            }
            let scores = handle_not_found({
                let mut r = client
                    .rosu
                    .user_scores(self.user.clone())
                    .limit(count)
                    .offset(offset);
                r = match self.score_type {
                    UserScoreType::Recent => r.recent().include_fails(self.include_fails),
                    UserScoreType::Best => r.best(),
                    UserScoreType::Pin => r.pinned(),
                };
                if let Some(mode) = self.mode {
                    r = r.mode(mode.into());
                }
                r.await
            })?
            .ok_or_else(|| error!("user not found"))?;
            let count = scores.len();
            Ok(Some((
                scores.into_iter().map(|v| v.into()).collect(),
                if count == MAXIMUM_LIMIT {
                    Some(offset + MAXIMUM_LIMIT)
                } else {
                    None
                },
            )))
        }

        pub(crate) fn build(
            self,
            client: OsuClient,
        ) -> impl TryStream<Ok = models::Score, Error = Error> {
            futures::stream::try_unfold(Some(0), move |off| {
                self.clone().with_offset(off, client.clone())
            })
            .map_ok(|v| futures::stream::iter(v).map(|v| Ok(v) as Result<_>))
            .try_flatten()
        }
    }
}

pub struct UserBestRequest {
    pub user: UserID,
    pub mode: Option<Mode>,
}
pub struct UserRecentRequest {
    pub user: UserID,
    pub mode: Option<Mode>,
}
