use super::*;
use sqlx::Executor;

/// An ignored user in the ignored user list.
#[derive(Clone, Debug)]
pub struct IgnoredUser {
    pub id: i64,
    pub username: String,
    pub ignored_since: DateTime,
}

impl IgnoredUser {
    /// Returns a list of all ignored users.
    pub async fn get_all<'a, E>(conn: E) -> Result<Vec<Self>>
    where
        E: Executor<'a, Database = Database>,
    {
        Ok(query_as!(
            IgnoredUser,
            r#"SELECT
              id,
              username,
              ignored_since as "ignored_since: DateTime"
            FROM ignored_users
            ORDER BY id ASC"#
        )
        .fetch_all(conn)
        .await?)
    }

    /// Add an user to ignore list.
    pub async fn add<'a, E>(conn: E, user_id: i64, username: String) -> Result<Self>
    where
        E: Executor<'a, Database = Database>,
    {
        Ok(query_as!(
            IgnoredUser,
            r#"INSERT INTO ignored_users(id, username) VALUES (?, ?)
               ON CONFLICT (id) DO UPDATE SET username = excluded.username
               RETURNING id,
              username,
              ignored_since as "ignored_since: DateTime""#,
            user_id,
            username
        )
        .fetch_one(conn)
        .await?)
    }

    // Remove an user from ignore list.
    pub async fn remove<'a, E>(conn: E, user_id: i64) -> Result<()>
    where
        E: Executor<'a, Database = Database>,
    {
        query!(r#"DELETE FROM ignored_users WHERE id = ?"#, user_id)
            .execute(conn)
            .await?;
        Ok(())
    }
}
