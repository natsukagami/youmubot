use chrono::{DateTime, Utc};
use serenity::model::id::UserId;
use std::collections::HashMap;
use youmubot_db::DB;
use youmubot_prelude::*;

/// A database map that stores an user with the respective handle.
pub type CfSavedUsers = DB<HashMap<UserId, CfUser>>;

/// A saved Codeforces user.
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct CfUser {
    pub handle: String,
    pub last_update: DateTime<Utc>,
    pub rating: Option<i64>,
}

impl Default for CfUser {
    fn default() -> Self {
        Self {
            handle: "".to_owned(),
            last_update: Utc::now(),
            rating: None,
        }
    }
}
