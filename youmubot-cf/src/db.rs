use chrono::{DateTime, Utc};
use codeforces::{RatingChange, User};
use serenity::model::id::UserId;
use std::collections::HashMap;
use youmubot_db::DB;

/// A database map that stores an user with the respective handle.
pub type CfSavedUsers = DB<HashMap<UserId, CfUser>>;

/// A saved Codeforces user.
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct CfUser {
    pub handle: String,
    pub last_update: DateTime<Utc>,
    #[serde(default)]
    pub last_contest_id: Option<u64>,
    pub rating: Option<i64>,
    #[serde(default)]
    pub failures: u8,
}

impl CfUser {
    /// Save a new user as an internal CFUser.
    /// Requires a vector of rating changes because we must rely on the Codeforces rating_changes API's return order to properly announce.
    pub(crate) fn save(u: User, rc: Vec<RatingChange>) -> Self {
        Self {
            handle: u.handle,
            last_update: Utc::now(),
            last_contest_id: rc.into_iter().last().map(|v| v.contest_id),
            rating: u.rating,
            failures: 0,
        }
    }
}
