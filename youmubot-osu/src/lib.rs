use std::collections::HashMap;
use std::convert::TryInto;
use std::sync::Arc;
use std::time::Duration;

use futures_util::lock::Mutex;
use leaky_bucket::RateLimiter;
use models::*;
use request::builders::*;
use request::scores::Fetch;
use request::*;
use tracing::debug;
use youmubot_prelude::*;

use crate::request::scores::FetchPure;

pub mod discord;
pub mod models;
pub mod request;

pub const MAX_TOP_SCORES_INDEX: usize = 200;

/// Client is the client that will perform calls to the osu! api server.
#[derive(Clone)]
pub struct OsuClient {
    rosu: Arc<Ratelimited<rosu_v2::Osu>>,

    user_header_cache: Arc<Mutex<HashMap<u64, Option<UserHeader>>>>,
}

pub(crate) struct Ratelimited<T> {
    inner: T,
    usage: Usage,
    limiter: RateLimiter,
}

impl<T> Ratelimited<T> {
    fn new(inner: T, usage: Usage) -> Self {
        let rpm = match usage {
            Usage::Background => 60,
            Usage::Foreground => 600,
        };
        let interval = Duration::from_millis(Duration::from_mins(1).as_millis() as u64 * 2 / rpm);
        let rl = RateLimiter::builder()
            .max(20)
            .interval(interval)
            .refill(2)
            .initial(0)
            .fair(true)
            .build();
        Ratelimited {
            inner,
            usage,
            limiter: rl,
        }
    }

    pub async fn acquire_one(&self) -> &T {
        self.acquire(1).await
    }

    pub async fn acquire(&self, tokens: usize) -> &T {
        debug!(
            "[{:?}] acquiring when bucket = {}",
            self.usage,
            self.limiter.balance()
        );
        self.limiter.acquire(tokens).await;
        debug!(
            "[{:?}] acquired when bucket = {}",
            self.usage,
            self.limiter.balance()
        );
        &self.inner
    }
}

pub fn vec_try_into<U, T: std::convert::TryFrom<U>>(v: Vec<U>) -> Result<Vec<T>, T::Error> {
    let mut res = Vec::with_capacity(v.len());

    for u in v.into_iter() {
        res.push(u.try_into()?);
    }

    Ok(res)
}

/// How is this used?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Usage {
    Background,
    Foreground,
}

impl OsuClient {
    /// Create a new client from the given API key.
    pub async fn new(
        client_id: u64,
        client_secret: impl Into<String>,
        usage: Usage,
    ) -> Result<OsuClient> {
        let rosu = rosu_v2::OsuBuilder::new()
            .client_id(client_id)
            .client_secret(client_secret)
            .build()
            .await?;
        Ok(OsuClient {
            rosu: Arc::new(Ratelimited::new(rosu, usage)),
            user_header_cache: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub async fn beatmaps(
        &self,
        kind: BeatmapRequestKind,
        f: impl FnOnce(&mut BeatmapRequestBuilder) -> &mut BeatmapRequestBuilder,
    ) -> Result<Vec<Beatmap>> {
        let mut r = BeatmapRequestBuilder::new(kind);
        f(&mut r);
        r.build(self).await
    }

    pub async fn user(
        &self,
        user: &UserID,
        f: impl FnOnce(&mut UserRequestBuilder) -> &mut UserRequestBuilder,
    ) -> Result<Option<User>, Error> {
        let mut r = UserRequestBuilder::new(user.clone());
        f(&mut r);
        let u = r.build(self).await?;
        if let UserID::ID(id) = user {
            self.user_header_cache
                .lock()
                .await
                .insert(*id, u.clone().map(|v| v.into()));
        }
        Ok(u)
    }

    /// Fetch user events for an user.
    pub async fn user_events(&self, user: UserID) -> Result<impl LazyBuffer<UserEvent>> {
        request::UserEventRequest { user }
            .as_fetch()
            .make_buffer(self.clone())
            .await
    }

    /// Fetch the user header.
    pub async fn user_header(&self, id: u64) -> Result<Option<UserHeader>, Error> {
        Ok({
            let v = self.user_header_cache.lock().await.get(&id).cloned();
            match v {
                Some(v) => v,
                None => self.user(&UserID::ID(id), |f| f).await?.map(|v| v.into()),
            }
        })
    }

    pub async fn scores(
        &self,
        beatmap_id: u64,
        f: impl FnOnce(&mut ScoreRequestBuilder) -> &mut ScoreRequestBuilder,
    ) -> Result<impl LazyBuffer<Score>> {
        let mut r = ScoreRequestBuilder::new(beatmap_id);
        f(&mut r);
        r.build(self).await
    }

    pub async fn user_best(
        &self,
        user: UserID,
        f: impl FnOnce(&mut UserScoreRequestBuilder) -> &mut UserScoreRequestBuilder,
    ) -> Result<impl LazyBuffer<Score>> {
        self.user_scores(UserScoreType::Best, user, f).await
    }

    pub async fn user_recent(
        &self,
        user: UserID,
        f: impl FnOnce(&mut UserScoreRequestBuilder) -> &mut UserScoreRequestBuilder,
    ) -> Result<impl LazyBuffer<Score>> {
        self.user_scores(UserScoreType::Recent, user, f).await
    }

    pub async fn user_pins(
        &self,
        user: UserID,
        f: impl FnOnce(&mut UserScoreRequestBuilder) -> &mut UserScoreRequestBuilder,
    ) -> Result<impl LazyBuffer<Score>> {
        self.user_scores(UserScoreType::Pin, user, f).await
    }

    async fn user_scores(
        &self,
        u: UserScoreType,
        user: UserID,
        f: impl FnOnce(&mut UserScoreRequestBuilder) -> &mut UserScoreRequestBuilder,
    ) -> Result<impl LazyBuffer<Score>> {
        let mut r = UserScoreRequestBuilder::new(u, user);
        f(&mut r);
        r.build(self.clone()).await
    }

    pub async fn score(&self, score_id: u64) -> Result<Option<Score>, Error> {
        let s = match self.rosu.acquire_one().await.score(score_id).await {
            Ok(v) => v,
            Err(rosu_v2::error::OsuError::NotFound) => return Ok(None),
            e => e?,
        };
        Ok(Some(s.into()))
    }
}
