pub mod models;

pub mod request;

#[cfg(test)]
mod test;

use models::*;
use request::builders::*;
use request::*;
use reqwest::Client as HTTPClient;
use serenity::framework::standard::CommandError as Error;

/// Client is the client that will perform calls to the osu! api server.
pub struct Client {
    key: String,
}

impl Client {
    /// Create a new client from the given API key.
    pub fn new(key: impl AsRef<str>) -> Client {
        Client {
            key: key.as_ref().to_string(),
        }
    }

    pub fn beatmaps(
        &self,
        client: &HTTPClient,
        kind: BeatmapRequestKind,
        f: impl FnOnce(&mut BeatmapRequestBuilder) -> &mut BeatmapRequestBuilder,
    ) -> Result<Vec<Beatmap>, Error> {
        let mut r = BeatmapRequestBuilder::new(kind);
        f(&mut r);
        let res = r.build(client).query(&[("k", &self.key)]).send()?.json()?;
        Ok(res)
    }
}
