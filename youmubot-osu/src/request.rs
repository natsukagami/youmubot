use crate::models::{Mode, Mods};
use crate::Client;
use chrono::{DateTime, Utc};
use youmubot_prelude::*;

trait ToQuery {
    fn to_query(&self) -> Vec<(&'static str, String)>;
}

impl<T: ToQuery> ToQuery for Option<T> {
    fn to_query(&self) -> Vec<(&'static str, String)> {
        match self {
            Some(ref v) => v.to_query(),
            None => vec![],
        }
    }
}

impl ToQuery for Mods {
    fn to_query(&self) -> Vec<(&'static str, String)> {
        vec![("mods", format!("{}", self.bits()))]
    }
}

impl ToQuery for Mode {
    fn to_query(&self) -> Vec<(&'static str, String)> {
        vec![("m", (*self as u8).to_string())]
    }
}

impl ToQuery for (Mode, bool) {
    fn to_query(&self) -> Vec<(&'static str, String)> {
        vec![
            ("m", (self.0 as u8).to_string()),
            ("a", (self.1 as u8).to_string()),
        ]
    }
}

impl ToQuery for (&'static str, String) {
    fn to_query(&self) -> Vec<(&'static str, String)> {
        vec![(self.0, self.1.clone())]
    }
}

impl ToQuery for (&'static str, DateTime<Utc>) {
    fn to_query(&self) -> Vec<(&'static str, String)> {
        vec![(self.0, format!("{}", self.1.format("%Y-%m-%d")))]
    }
}

pub enum UserID {
    Username(String),
    ID(u64),
    Auto(String),
}

impl ToQuery for UserID {
    fn to_query(&self) -> Vec<(&'static str, String)> {
        use UserID::*;
        match self {
            Username(ref s) => vec![("u", s.clone()), ("type", "string".to_owned())],
            ID(u) => vec![("u", u.to_string()), ("type", "id".to_owned())],
            Auto(ref s) => vec![("u", s.clone())],
        }
    }
}
pub enum BeatmapRequestKind {
    Beatmap(u64),
    Beatmapset(u64),
    BeatmapHash(String),
}

impl ToQuery for BeatmapRequestKind {
    fn to_query(&self) -> Vec<(&'static str, String)> {
        use BeatmapRequestKind::*;
        match self {
            Beatmap(b) => vec![("b", b.to_string())],
            Beatmapset(s) => vec![("s", s.to_string())],
            BeatmapHash(ref h) => vec![("h", h.clone())],
        }
    }
}

pub mod builders {
    use reqwest::Response;

    use crate::models;

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

        pub(crate) async fn build(self, client: &Client) -> Result<Vec<models::Beatmap>> {
            Ok(match self.kind {
                BeatmapRequestKind::Beatmap(id) => {
                    let mut bm = client.rosu.beatmap().map_id(id as u32).await?;
                    let set = bm.mapset.take().unwrap();
                    vec![models::Beatmap::from_rosu(bm, &set)]
                }
                BeatmapRequestKind::Beatmapset(id) => {
                    let mut set = client.rosu.beatmapset(id as u32).await?;
                    let bms = set.maps.take().unwrap();
                    bms.into_iter()
                        .map(|bm| models::Beatmap::from_rosu(bm, &set))
                        .collect()
                }
                BeatmapRequestKind::BeatmapHash(hash) => {
                    let mut bm = client.rosu.beatmap().checksum(hash).await?;
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

        pub fn mode(&mut self, mode: Mode) -> &mut Self {
            self.mode = Some(mode);
            self
        }

        pub fn event_days(&mut self, event_days: u8) -> &mut Self {
            self.event_days = Some(event_days).filter(|&v| v <= 31).or(self.event_days);
            self
        }

        pub(crate) async fn build(&self, client: &Client) -> Result<Response> {
            Ok(client
                .build_request("https://osu.ppy.sh/api/get_user")
                .await?
                .query(&self.user.to_query())
                .query(&self.mode.to_query())
                .query(
                    &self
                        .event_days
                        .map(|v| ("event_days", v.to_string()))
                        .to_query(),
                )
                .send()
                .await?)
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

        pub fn mode(&mut self, mode: Mode) -> &mut Self {
            self.mode = Some(mode);
            self
        }

        pub fn mods(&mut self, mods: Mods) -> &mut Self {
            self.mods = Some(mods);
            self
        }

        pub fn limit(&mut self, limit: u8) -> &mut Self {
            self.limit = Some(limit).filter(|&v| v <= 100).or(self.limit);
            self
        }

        pub(crate) async fn build(&self, client: &Client) -> Result<Response> {
            Ok(client
                .build_request("https://osu.ppy.sh/api/get_scores")
                .await?
                .query(&[("b", self.beatmap_id)])
                .query(&self.user.to_query())
                .query(&self.mode.to_query())
                .query(&self.mods.to_query())
                .query(&self.limit.map(|v| ("limit", v.to_string())).to_query())
                .send()
                .await?)
        }
    }

    pub(crate) enum UserScoreType {
        Recent,
        Best,
    }

    pub struct UserScoreRequestBuilder {
        score_type: UserScoreType,
        user: UserID,
        mode: Option<Mode>,
        limit: Option<u8>,
    }

    impl UserScoreRequestBuilder {
        pub(crate) fn new(score_type: UserScoreType, user: UserID) -> Self {
            UserScoreRequestBuilder {
                score_type,
                user,
                mode: None,
                limit: None,
            }
        }

        pub fn mode(&mut self, m: Mode) -> &mut Self {
            self.mode = Some(m);
            self
        }

        pub fn limit(&mut self, limit: u8) -> &mut Self {
            self.limit = Some(limit).filter(|&v| v <= 100).or(self.limit);
            self
        }

        pub(crate) async fn build(&self, client: &Client) -> Result<Response> {
            Ok(client
                .build_request(match self.score_type {
                    UserScoreType::Best => "https://osu.ppy.sh/api/get_user_best",
                    UserScoreType::Recent => "https://osu.ppy.sh/api/get_user_recent",
                })
                .await?
                .query(&self.user.to_query())
                .query(&self.mode.to_query())
                .query(&self.limit.map(|v| ("limit", v.to_string())).to_query())
                .send()
                .await?)
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
