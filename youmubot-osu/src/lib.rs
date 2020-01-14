pub mod models;

pub mod request;

#[cfg(test)]
mod test;

use models::*;
use request::builders::*;
use request::*;
use reqwest::Client as HTTPClient;
use serenity::framework::standard::CommandError as Error;
use std::convert::TryInto;

/// Client is the client that will perform calls to the osu! api server.
pub struct Client {
    key: String,
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
    pub fn new(key: impl AsRef<str>) -> Client {
        Client {
            key: key.as_ref().to_string(),
        }
    }

    fn build_request(
        &self,
        c: &HTTPClient,
        r: reqwest::RequestBuilder,
    ) -> Result<reqwest::Response, Error> {
        let v = r.query(&[("k", &self.key)]).build()?;
        dbg!(v.url());
        Ok(c.execute(v)?)
    }

    pub fn beatmaps(
        &self,
        client: &HTTPClient,
        kind: BeatmapRequestKind,
        f: impl FnOnce(&mut BeatmapRequestBuilder) -> &mut BeatmapRequestBuilder,
    ) -> Result<Vec<Beatmap>, Error> {
        let mut r = BeatmapRequestBuilder::new(kind);
        f(&mut r);
        let res: Vec<raw::Beatmap> = self.build_request(client, r.build(client))?.json()?;
        Ok(vec_try_into(res)?)
    }

    pub fn user(
        &self,
        client: &HTTPClient,
        user: UserID,
        f: impl FnOnce(&mut UserRequestBuilder) -> &mut UserRequestBuilder,
    ) -> Result<Option<User>, Error> {
        let mut r = UserRequestBuilder::new(user);
        f(&mut r);
        let res: Vec<raw::User> = self.build_request(client, r.build(client))?.json()?;
        let res = vec_try_into(res)?;
        Ok(res.into_iter().next())
    }

    pub fn scores(
        &self,
        client: &HTTPClient,
        beatmap_id: u64,
        f: impl FnOnce(&mut ScoreRequestBuilder) -> &mut ScoreRequestBuilder,
    ) -> Result<Vec<Score>, Error> {
        let mut r = ScoreRequestBuilder::new(beatmap_id);
        f(&mut r);
        let res: Vec<raw::Score> = self.build_request(client, r.build(client))?.json()?;
        let mut res: Vec<Score> = vec_try_into(res)?;

        // with a scores request you need to fill the beatmap ids yourself
        res.iter_mut().for_each(|v| {
            v.beatmap_id = beatmap_id;
        });
        Ok(res)
    }

    pub fn user_best(
        &self,
        client: &HTTPClient,
        user: UserID,
        f: impl FnOnce(&mut UserScoreRequestBuilder) -> &mut UserScoreRequestBuilder,
    ) -> Result<Vec<Score>, Error> {
        self.user_scores(UserScoreType::Best, client, user, f)
    }

    pub fn user_recent(
        &self,
        client: &HTTPClient,
        user: UserID,
        f: impl FnOnce(&mut UserScoreRequestBuilder) -> &mut UserScoreRequestBuilder,
    ) -> Result<Vec<Score>, Error> {
        self.user_scores(UserScoreType::Recent, client, user, f)
    }

    fn user_scores(
        &self,
        u: UserScoreType,
        client: &HTTPClient,
        user: UserID,
        f: impl FnOnce(&mut UserScoreRequestBuilder) -> &mut UserScoreRequestBuilder,
    ) -> Result<Vec<Score>, Error> {
        let mut r = UserScoreRequestBuilder::new(u, user);
        f(&mut r);
        let res: Vec<raw::Score> = self.build_request(client, r.build(client))?.json()?;
        let res = vec_try_into(res)?;
        Ok(res)
    }
}
