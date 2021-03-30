/// The DateTime used in the package.
pub type DateTime = chrono::DateTime<chrono::Utc>;

use futures_util::stream::{Stream, StreamExt, TryStreamExt};

pub mod osu_user;
