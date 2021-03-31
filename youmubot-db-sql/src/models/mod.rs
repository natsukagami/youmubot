/// The DateTime used in the package.
pub type DateTime = chrono::DateTime<chrono::Utc>;

use crate::*;
use futures_util::stream::{Stream, StreamExt, TryStreamExt};
use sqlx::{query, query_as, Executor};

pub mod osu;
pub mod osu_user;

/// Map a `fetch_many` result to a normal result.
pub(crate) async fn map_many_result<T, E, W>(
    item: Result<either::Either<W, T>, E>,
) -> Option<Result<T>>
where
    E: Into<Error>,
{
    match item {
        Ok(v) => v.right().map(Ok),
        Err(e) => Some(Err(e.into())),
    }
}
