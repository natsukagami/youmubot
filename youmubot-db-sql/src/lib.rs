use sqlx::sqlite;
use std::path::Path;

pub use errors::*;

/// The DB constructs that will be used in the package.
pub use sqlite::{SqliteConnection as Connection, SqliteError, SqlitePool as Pool};
pub use sqlx::Sqlite as Database;

/// Models defined in the database.
pub mod models;

/// Create a new pool of sqlite connections to the given database path,
/// run migrations on it and return the result.
pub async fn connect(path: impl AsRef<Path>) -> Result<Pool> {
    let pool = Pool::connect_with(
        sqlite::SqliteConnectOptions::new()
            .filename(path)
            .foreign_keys(true)
            .create_if_missing(true)
            .journal_mode(sqlite::SqliteJournalMode::Wal)
            .synchronous(sqlite::SqliteSynchronous::Normal),
    )
    .await?;

    // Run migration before we return.
    migration::MIGRATOR.run(&pool).await?;

    Ok(pool)
}

pub mod errors {
    /// Default `Result` type used in this package.
    pub type Result<T, E = Error> = std::result::Result<T, E>;
    /// Possible errors in the package.
    #[derive(thiserror::Error, Debug)]
    pub enum Error {
        #[error("sqlx error: {:?}", .0)]
        SQLx(sqlx::Error),
        #[error("sqlx migration error: {:?}", .0)]
        Migration(#[from] sqlx::migrate::MigrateError),
        #[error("values already existed for: {}", .0)]
        Duplicate(String),
    }

    impl From<sqlx::Error> for Error {
        fn from(value: sqlx::Error) -> Self {
            match value {
                // if we can match a constraint error, give it a special case.
                sqlx::Error::Database(database_error) => {
                    let msg = database_error.message();
                    match msg.strip_prefix("UNIQUE constraint failed: ") {
                        Some(con) => Error::Duplicate(con.to_owned()),
                        None => Error::SQLx(sqlx::Error::Database(database_error)),
                    }
                }
                e => Error::SQLx(e),
            }
        }
    }
}

mod migration {
    use sqlx::migrate::Migrator;

    pub(crate) static MIGRATOR: Migrator = sqlx::migrate!("./migrations");
}
