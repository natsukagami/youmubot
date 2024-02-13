pub mod discord;
pub mod models;
pub mod request;

#[cfg(test)]
mod test;

use models::*;
use request::builders::*;
use request::*;
use std::convert::TryInto;
use youmubot_prelude::*;

/// Client is the client that will perform calls to the osu! api server.
pub struct Client {
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
    pub async fn new(client_id: u64, client_secret: impl Into<String>) -> Result<Client> {
        let rosu = rosu_v2::OsuBuilder::new()
            .client_id(client_id)
            .client_secret(client_secret)
            .build()
            .await?;
        Ok(Client { rosu })
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
        r.build(self).await
    }

    pub async fn scores(
        &self,
        beatmap_id: u64,
        f: impl FnOnce(&mut ScoreRequestBuilder) -> &mut ScoreRequestBuilder,
    ) -> Result<Vec<Score>, Error> {
        let mut r = ScoreRequestBuilder::new(beatmap_id);
        f(&mut r);
        r.build(self).await
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
        r.build(self).await
    }
}
