pub mod discord;
pub mod models;
pub mod request;

#[cfg(test)]
mod test;

use models::*;
use request::builders::*;
use request::*;
use reqwest::Client as HTTPClient;
use std::convert::TryInto;
use youmubot_prelude::{ratelimit::Ratelimit, *};

/// The number of requests per minute to the osu! server.
const REQUESTS_PER_MINUTE: usize = 100;

/// Client is the client that will perform calls to the osu! api server.
pub struct Client {
    client: Ratelimit<HTTPClient>,
    key: String,

    rosu: rosu_v2::Osu,
}

pub fn vec_try_into<U, T: std::convert::TryFrom<U>>(v: Vec<U>) -> Result<Vec<T>, T::Error> {
    let mut res = Vec::with_capacity(v.len());

    for u in v.into_iter() {
        res.push(u.try_into()?);
    }

    Ok(res)
}

impl Client {
    /// Create a new client from the given API key.
    pub async fn new(
        key: String,
        client: HTTPClient,
        client_id: u64,
        client_secret: impl Into<String>,
    ) -> Result<Client> {
        let client = Ratelimit::new(
            client,
            REQUESTS_PER_MINUTE,
            std::time::Duration::from_secs(60),
        );
        let rosu = rosu_v2::OsuBuilder::new()
            .client_id(client_id)
            .client_secret(client_secret)
            .build()
            .await?;
        Ok(Client { client, key, rosu })
    }

    pub(crate) async fn build_request(&self, url: &str) -> Result<reqwest::RequestBuilder> {
        Ok(self
            .client
            .borrow()
            .await?
            .get(url)
            .query(&[("k", &*self.key)]))
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
        user: UserID,
        f: impl FnOnce(&mut UserRequestBuilder) -> &mut UserRequestBuilder,
    ) -> Result<Option<User>, Error> {
        let mut r = UserRequestBuilder::new(user);
        f(&mut r);
        let res: Vec<raw::User> = r.build(self).await?.json().await?;
        let res = vec_try_into(res)?;
        Ok(res.into_iter().next())
    }

    pub async fn scores(
        &self,
        beatmap_id: u64,
        f: impl FnOnce(&mut ScoreRequestBuilder) -> &mut ScoreRequestBuilder,
    ) -> Result<Vec<Score>, Error> {
        let mut r = ScoreRequestBuilder::new(beatmap_id);
        f(&mut r);
        let res: Vec<raw::Score> = r.build(self).await?.json().await?;
        let mut res: Vec<Score> = vec_try_into(res)?;

        // with a scores request you need to fill the beatmap ids yourself
        res.iter_mut().for_each(|v| {
            v.beatmap_id = beatmap_id;
        });
        Ok(res)
    }

    pub async fn user_best(
        &self,
        user: UserID,
        f: impl FnOnce(&mut UserScoreRequestBuilder) -> &mut UserScoreRequestBuilder,
    ) -> Result<Vec<Score>, Error> {
        self.user_scores(UserScoreType::Best, user, f).await
    }

    pub async fn user_recent(
        &self,
        user: UserID,
        f: impl FnOnce(&mut UserScoreRequestBuilder) -> &mut UserScoreRequestBuilder,
    ) -> Result<Vec<Score>, Error> {
        self.user_scores(UserScoreType::Recent, user, f).await
    }

    async fn user_scores(
        &self,
        u: UserScoreType,
        user: UserID,
        f: impl FnOnce(&mut UserScoreRequestBuilder) -> &mut UserScoreRequestBuilder,
    ) -> Result<Vec<Score>, Error> {
        let mut r = UserScoreRequestBuilder::new(u, user);
        f(&mut r);
        let res: Vec<raw::Score> = r.build(self).await?.json().await?;
        let res = vec_try_into(res)?;
        Ok(res)
    }
}
