use core::fmt;
use std::sync::Arc;

use crate::models::{Mode, Mods, UserEvent};
use crate::OsuClient;
use rosu_v2::error::OsuError;
use scores::Fetch;
use youmubot_prelude::*;

pub(crate) mod scores;

pub use scores::LazyBuffer;

#[derive(Clone, Debug)]
pub enum UserID {
    Username(Arc<String>),
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
            UserID::Username(s) => rosu_v2::request::UserId::Name(s[..].into()),
            UserID::ID(id) => rosu_v2::request::UserId::Id(id as u32),
        }
    }
}

impl UserID {
    pub fn from_string(s: impl Into<String>) -> UserID {
        let s = s.into();
        match s.parse::<u64>() {
            Ok(id) => UserID::ID(id),
            Err(_) => UserID::Username(Arc::new(s)),
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
    use rosu_v2::model::mods::GameModsIntermode;

    use crate::models::{self, Score};

    use super::scores::Fetch;
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
    }

    impl UserRequestBuilder {
        pub(crate) fn new(user: UserID) -> Self {
            UserRequestBuilder { user, mode: None }
        }

        pub fn mode(&mut self, mode: impl Into<Option<Mode>>) -> &mut Self {
            self.mode = mode.into();
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
            let stats = user.statistics.take().unwrap();
            Ok(Some(models::User::from_rosu(user, stats)))
        }
    }

    pub struct ScoreRequestBuilder {
        beatmap_id: u64,
        user: Option<UserID>,
        mode: Option<Mode>,
        mods: Option<Mods>,
    }

    impl ScoreRequestBuilder {
        pub(crate) fn new(beatmap_id: u64) -> Self {
            ScoreRequestBuilder {
                beatmap_id,
                user: None,
                mode: None,
                mods: None,
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

        async fn fetch_scores(
            &self,
            osu: &crate::OsuClient,
            _offset: usize,
        ) -> Result<Vec<models::Score>> {
            let scores = handle_not_found(match &self.user {
                Some(user) => {
                    let mut r = osu
                        .rosu
                        .beatmap_user_scores(self.beatmap_id as u32, user.clone());
                    if let Some(mode) = self.mode {
                        r = r.mode(mode.into());
                    }
                    match &self.mods {
                        Some(mods) => r.await.map(|mut ss| {
                            ss.retain(|s| {
                                Mods::from_gamemods(s.mods.clone(), s.set_on_lazer).contains(mods)
                            });
                            ss
                        }),
                        None => r.await,
                    }
                }
                None => {
                    let mut r = osu.rosu.beatmap_scores(self.beatmap_id as u32).global();
                    if let Some(mode) = &self.mode {
                        r = r.mode((*mode).into());
                    }
                    if let Some(mods) = &self.mods {
                        r = r.mods(GameModsIntermode::from(mods.inner.clone()));
                    }
                    // r = r.limit(limit); // can't do this just yet because of offset not working
                    r.await
                }
            })?
            .ok_or_else(|| error!("beatmap or user not found"))?;
            Ok(scores.into_iter().map(|v| v.into()).collect())
        }

        pub(crate) async fn build(self, osu: &OsuClient) -> Result<impl LazyBuffer<Score>> {
            // user queries always return all scores, so no need to consider offset.
            // otherwise, it's not working anyway...
            self.fetch_scores(osu, 0).await
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
        include_fails: bool,
    }

    impl UserScoreRequestBuilder {
        pub(crate) fn new(score_type: UserScoreType, user: UserID) -> Self {
            UserScoreRequestBuilder {
                score_type,
                user,
                mode: None,
                include_fails: true,
            }
        }

        pub fn mode(&mut self, m: Mode) -> &mut Self {
            self.mode = Some(m);
            self
        }

        pub fn include_fails(&mut self, include_fails: bool) -> &mut Self {
            self.include_fails = include_fails;
            self
        }

        const SCORES_PER_PAGE: usize = 100;

        async fn with_offset(&self, client: &OsuClient, offset: usize) -> Result<Vec<Score>> {
            let scores = handle_not_found({
                let mut r = client
                    .rosu
                    .user_scores(self.user.clone())
                    .limit(Self::SCORES_PER_PAGE)
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
            Ok(scores.into_iter().map(|v| v.into()).collect())
        }

        pub(crate) async fn build(self, client: OsuClient) -> Result<impl LazyBuffer<Score>> {
            self.make_buffer(client).await
        }
    }

    impl Fetch for UserScoreRequestBuilder {
        type Item = Score;
        async fn fetch(&self, client: &crate::OsuClient, offset: usize) -> Result<Vec<Score>> {
            self.with_offset(client, offset).await
        }

        const ITEMS_PER_PAGE: usize = Self::SCORES_PER_PAGE;
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

pub struct UserEventRequest {
    pub user: UserID,
}

impl Fetch for UserEventRequest {
    type Item = UserEvent;
    const ITEMS_PER_PAGE: usize = 50;

    async fn fetch(&self, client: &crate::OsuClient, offset: usize) -> Result<Vec<Self::Item>> {
        Ok(handle_not_found(
            client
                .rosu
                .recent_activity(self.user.clone())
                .limit(Self::ITEMS_PER_PAGE)
                .offset(offset)
                .await,
        )?
        .ok_or_else(|| error!("user not found"))?
        .into_iter()
        .map(Into::into)
        .collect())
    }
}
