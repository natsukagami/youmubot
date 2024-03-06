use super::*;
use sqlx::{query, query_as, Executor};

/// An osu user, as represented in the SQL.
#[derive(Debug, Clone)]
pub struct OsuUser {
    pub user_id: i64,
    pub username: Option<String>, // should always be there
    pub id: i64,
    pub last_update: DateTime,
    pub pp_std: Option<f64>,
    pub pp_taiko: Option<f64>,
    pub pp_mania: Option<f64>,
    pub pp_catch: Option<f64>,
    /// Number of consecutive update failures
    pub failures: u8,

    pub std_weighted_map_length: Option<f64>,
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
                username,
                id as "id: i64",
                last_update as "last_update: DateTime",
                pp_std, pp_taiko, pp_mania, pp_catch,
                failures as "failures: u8",
                std_weighted_map_length
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
                username,
                id as "id: i64",
                last_update as "last_update: DateTime",
                pp_std, pp_taiko, pp_mania, pp_catch,
                failures as "failures: u8",
                std_weighted_map_length
            FROM osu_users WHERE id = ?"#,
            osu_id
        )
        .fetch_optional(conn)
        .await?;
        Ok(u)
    }

    /// Query all users.
    pub fn all<'a, E>(conn: &'a mut E) -> impl Stream<Item = Result<Self>> + 'a
    where
        &'a mut E: Executor<'a, Database = Database>,
    {
        query_as!(
            Self,
            r#"SELECT
                user_id as "user_id: i64",
                username,
                id as "id: i64",
                last_update as "last_update: DateTime",
                pp_std, pp_taiko, pp_mania, pp_catch,
                failures as "failures: u8",
                std_weighted_map_length
            FROM osu_users"#,
        )
        .fetch_many(conn)
        .filter_map(map_many_result)
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
               INTO osu_users(user_id, username, id, last_update, pp_std, pp_taiko, pp_mania, pp_catch, failures, std_weighted_map_length)
               VALUES(?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
               ON CONFLICT (user_id) WHERE id = ? DO UPDATE
               SET
                last_update = excluded.last_update,
                username = excluded.username,
                pp_std = excluded.pp_std,
                pp_taiko = excluded.pp_taiko,
                pp_mania = excluded.pp_mania,
                pp_catch = excluded.pp_catch,
                failures = excluded.failures,
                std_weighted_map_length = excluded.std_weighted_map_length
            "#,
            self.user_id,
            self.username,
            self.id,
            self.last_update,
            self.pp_std,
            self.pp_taiko,
            self.pp_mania,
            self.pp_catch,
            self.failures,
            self.std_weighted_map_length,

            self.user_id,
        ).execute(conn).await?;
        Ok(())
    }

    pub async fn delete(user_id: i64, conn: impl Executor<'_, Database = Database>) -> Result<()> {
        query!("DELETE FROM osu_users WHERE user_id = ?", user_id)
            .execute(conn)
            .await?;
        Ok(())
    }
}
