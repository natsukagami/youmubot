use crate::*;
use sqlx::{query, query_as, Executor};

/// The DateTime used in the package.
pub type DateTime = chrono::DateTime<chrono::Utc>;

pub mod osu;
pub mod osu_user;
