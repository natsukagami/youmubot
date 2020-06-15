pub mod discord;
pub mod models;
pub mod request;

#[cfg(test)]
mod test;

use models::*;
use request::builders::*;
use request::*;
use reqwest::blocking::{Client as HTTPClient, RequestBuilder, Response};
use serenity::framework::standard::CommandError as Error;
use std::{convert::TryInto, sync::Arc};

/// Client is the client that will perform calls to the osu! api server.
/// It's cheap to clone, so do it.
#[derive(Clone, Debug)]
pub struct Client {
    key: Arc<String>,
    client: HTTPClient,
}

fn vec_try_into<U, T: std::convert::TryFrom<U>>(v: Vec<U>) -> Result<Vec<T>, T::Error> {
    let mut res = Vec::with_capacity(v.len());

    for u in v.into_iter() {
        res.push(u.try_into()?);
    }

    Ok(res)
}

impl Client {
    /// Create a new client from the given API key.
    pub fn new(http_client: HTTPClient, key: String) -> Client {
        Client {
            key: Arc::new(key),
            client: http_client,
        }
    }

    fn build_request(&self, r: RequestBuilder) -> Result<Response, Error> {
        let v = r.query(&[("k", &*self.key)]).build()?;
        // dbg!(v.url());
        Ok(self.client.execute(v)?)
    }

    pub fn beatmaps(
        &self,
        kind: BeatmapRequestKind,
        f: impl FnOnce(&mut BeatmapRequestBuilder) -> &mut BeatmapRequestBuilder,
    ) -> Result<Vec<Beatmap>, Error> {
        let mut r = BeatmapRequestBuilder::new(kind);
        f(&mut r);
        let res: Vec<raw::Beatmap> = self.build_request(r.build(&self.client))?.json()?;
        Ok(vec_try_into(res)?)
    }

    pub fn user(
        &self,
        user: UserID,
        f: impl FnOnce(&mut UserRequestBuilder) -> &mut UserRequestBuilder,
    ) -> Result<Option<User>, Error> {
        let mut r = UserRequestBuilder::new(user);
        f(&mut r);
        let res: Vec<raw::User> = self.build_request(r.build(&self.client))?.json()?;
        let res = vec_try_into(res)?;
        Ok(res.into_iter().next())
    }

    pub fn scores(
        &self,
        beatmap_id: u64,
        f: impl FnOnce(&mut ScoreRequestBuilder) -> &mut ScoreRequestBuilder,
    ) -> Result<Vec<Score>, Error> {
        let mut r = ScoreRequestBuilder::new(beatmap_id);
        f(&mut r);
        let res: Vec<raw::Score> = self.build_request(r.build(&self.client))?.json()?;
        let mut res: Vec<Score> = vec_try_into(res)?;

        // with a scores request you need to fill the beatmap ids yourself
        res.iter_mut().for_each(|v| {
            v.beatmap_id = beatmap_id;
        });
        Ok(res)
    }

    pub fn user_best(
        &self,
        user: UserID,
        f: impl FnOnce(&mut UserScoreRequestBuilder) -> &mut UserScoreRequestBuilder,
    ) -> Result<Vec<Score>, Error> {
        self.user_scores(UserScoreType::Best, user, f)
    }

    pub fn user_recent(
        &self,
        user: UserID,
        f: impl FnOnce(&mut UserScoreRequestBuilder) -> &mut UserScoreRequestBuilder,
    ) -> Result<Vec<Score>, Error> {
        self.user_scores(UserScoreType::Recent, user, f)
    }

    fn user_scores(
        &self,
        u: UserScoreType,
        user: UserID,
        f: impl FnOnce(&mut UserScoreRequestBuilder) -> &mut UserScoreRequestBuilder,
    ) -> Result<Vec<Score>, Error> {
        let mut r = UserScoreRequestBuilder::new(u, user);
        f(&mut r);
        let res: Vec<raw::Score> = self.build_request(r.build(&self.client))?.json()?;
        let res = vec_try_into(res)?;
        Ok(res)
    }
}
