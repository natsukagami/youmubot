use crate::*;
use futures_util::stream::{Stream, StreamExt};
use sqlx::{query, query_as, Executor};

/// The DateTime used in the package.
pub type DateTime = chrono::DateTime<chrono::Utc>;

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
