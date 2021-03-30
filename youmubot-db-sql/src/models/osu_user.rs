use super::*;
use crate::*;
use sqlx::{query, query_as, Executor};

/// An osu user, as represented in the SQL.
#[derive(Debug, Clone)]
pub struct OsuUser {
    pub user_id: i64,
    pub id: i64,
    pub last_update: Option<DateTime>,
    pub pp_std: Option<f32>,
    pub pp_taiko: Option<f32>,
    pub pp_mania: Option<f32>,
    pub pp_catch: Option<f32>,
    /// Number of consecutive update failures
    pub failures: u8,
}

impl OsuUser {
    /// Query an user by their user id.
    pub async fn by_user_id<'a, E>(user_id: i64, conn: &'a mut E) -> Result<Option<Self>>
    where
        &'a mut E: Executor<'a, Database = Database>,
    {
        let u = query_as!(
            Self,
            r#"SELECT
                user_id as "user_id: i64",
                id as "id: i64",
                last_update as "last_update: DateTime",
                pp_std, pp_taiko, pp_mania, pp_catch,
                failures as "failures: u8"
            FROM osu_users WHERE user_id = ?"#,
            user_id
        )
        .fetch_optional(conn)
        .await?;
        Ok(u)
    }

    /// Query an user by their osu id.
    pub async fn by_osu_id<'a, E>(osu_id: i64, conn: &'a mut E) -> Result<Option<Self>>
    where
        &'a mut E: Executor<'a, Database = Database>,
    {
        let u = query_as!(
            Self,
            r#"SELECT
                user_id as "user_id: i64",
                id as "id: i64",
                last_update as "last_update: DateTime",
                pp_std, pp_taiko, pp_mania, pp_catch,
                failures as "failures: u8"
            FROM osu_users WHERE id = ?"#,
            osu_id
        )
        .fetch_optional(conn)
        .await?;
        Ok(u)
    }

    /// Query all users.
    pub async fn all<'a, E>(conn: &'a mut E) -> Result<impl Stream<Item = Result<Self>> + 'a>
    where
        &'a mut E: Executor<'a, Database = Database>,
    {
        let u = query_as!(
            Self,
            r#"SELECT
                user_id as "user_id: i64",
                id as "id: i64",
                last_update as "last_update: DateTime",
                pp_std, pp_taiko, pp_mania, pp_catch,
                failures as "failures: u8"
            FROM osu_users"#,
        )
        .fetch_many(conn)
        .filter_map(|either| {
            futures_util::future::ready(match either {
                Ok(v) => v.right().map(Ok),
                Err(e) => Some(Err(Error::from(e))),
            })
        });
        Ok(u)
    }
}

impl OsuUser {
    /// Stores the user.
    pub async fn store<'a, E>(&self, conn: &'a mut E) -> Result<()>
    where
        &'a mut E: Executor<'a, Database = Database>,
    {
        query!(
            r#"INSERT
               INTO osu_users(user_id, id, last_update, pp_std, pp_taiko, pp_mania, pp_catch, failures)
               VALUES(?, ?, ?, ?, ?, ?, ?, ?)
               ON CONFLICT (user_id) DO UPDATE
               SET
                id = excluded.id,
                last_update = excluded.last_update,
                pp_std = excluded.pp_std,
                pp_taiko = excluded.pp_taiko,
                pp_mania = excluded.pp_mania,
                pp_catch = excluded.pp_catch,
                failures = excluded.failures
            "#,
            self.user_id,
            self.id,
            self.last_update,
            self.pp_std,
            self.pp_taiko,
            self.pp_mania,
            self.pp_catch,
            self.failures).execute(conn).await?;
        Ok(())
    }
}
