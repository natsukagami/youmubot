use chrono::{DateTime, Utc};

use crate::models::{Beatmap, Mode, Score};
use serde::{Deserialize, Serialize};
use serenity::model::id::{ChannelId, UserId};
use std::collections::HashMap;
use youmubot_db::DB;

/// Save the user IDs.
pub type OsuSavedUsers = DB<HashMap<UserId, OsuUser>>;

/// Save each channel's last requested beatmap.
pub type OsuLastBeatmap = DB<HashMap<ChannelId, (Beatmap, Mode)>>;

/// Save each beatmap's plays by user.
pub type OsuUserBests =
    DB<HashMap<(u64, Mode) /* Beatmap ID and Mode */, HashMap<UserId, Vec<Score>>>>;

/// An osu! saved user.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OsuUser {
    pub id: u64,
    pub last_update: DateTime<Utc>,
    #[serde(default)]
    pub pp: Vec<Option<f64>>,
}
