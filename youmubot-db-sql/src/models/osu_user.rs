use super::*;
use sqlx::{query, query_as, Executor, Transaction};
use std::collections::HashMap as Map;

/// An osu user, as represented in the SQL.
#[derive(Debug, Clone)]
pub struct OsuUser {
    pub user_id: i64,
    pub username: Option<String>, // should always be there
    pub id: i64,
    pub modes: Map<u8, OsuUserMode>,
    pub preferred_mode: u8,
    /// Number of consecutive update failures
    pub failures: u8,
}

/// Stats for a single user and mode.
#[derive(Debug, Clone)]
pub struct OsuUserMode {
    pub pp: f64,
    pub map_length: f64,
    pub map_age: i64,
    pub last_update: DateTime,
}

impl OsuUserMode {
    async fn from_user<'a, E>(id: i64, conn: E) -> Result<Map<u8, Self>>
    where
        E: Executor<'a, Database = Database>,
    {
        Ok(query!(
            r#"SELECT
              mode as "mode: u8",
              pp,
              map_length,
              map_age,
              last_update as "last_update: DateTime"
            FROM osu_user_mode_stats
            WHERE user_id = ?
            ORDER BY mode ASC"#,
            id
        )
        .fetch_all(conn)
        .await?
        .into_iter()
        .map(|row| {
            (
                row.mode,
                Self {
                    pp: row.pp,
                    map_length: row.map_length,
                    map_age: row.map_age,
                    last_update: row.last_update,
                },
            )
        })
        .collect())
    }

    async fn fetch_all<'a, E>(conn: E) -> Result<Map<i64, Map<u8, Self>>>
    where
        E: Executor<'a, Database = Database>,
    {
        let mut res: Map<i64, Map<u8, Self>> = Map::new();
        query!(
            r#"SELECT
              user_id as "user_id: i64",
              mode as "mode: u8",
              pp,
              map_length,
              map_age,
              last_update as "last_update: DateTime"
            FROM osu_user_mode_stats
            ORDER BY user_id ASC, mode ASC"#,
        )
        .fetch_all(conn)
        .await?
        .into_iter()
        .for_each(|v| {
            let modes = res.entry(v.user_id).or_default();
            modes.insert(
                v.mode,
                Self {
                    pp: v.pp,
                    map_length: v.map_length,
                    map_age: v.map_age,
                    last_update: v.last_update,
                },
            );
        });
        Ok(res)
    }
}

mod raw {
    #[derive(Debug)]
    pub struct OsuUser {
        pub user_id: i64,
        pub username: Option<String>, // should always be there
        pub id: i64,
        pub preferred_mode: u8,
        pub failures: u8,
    }
}

impl OsuUser {
    fn from_raw(r: raw::OsuUser, modes: Map<u8, OsuUserMode>) -> Self {
        Self {
            user_id: r.user_id,
            username: r.username,
            id: r.id,
            modes,
            preferred_mode: r.preferred_mode,
            failures: r.failures,
        }
    }
    /// Query an user by their user id.
    pub async fn by_user_id(
        user_id: i64,
        conn: &mut Transaction<'_, Database>,
    ) -> Result<Option<Self>> {
        let u = match query_as!(
            raw::OsuUser,
            r#"SELECT
                user_id as "user_id: i64",
                username,
                id as "id: i64",
                preferred_mode as "preferred_mode: u8",
                failures as "failures: u8"
            FROM osu_users WHERE user_id = ?"#,
            user_id
        )
        .fetch_optional(&mut **conn)
        .await?
        {
            Some(v) => v,
            None => return Ok(None),
        };
        let modes = OsuUserMode::from_user(u.user_id, &mut **conn).await?;
        Ok(Some(Self::from_raw(u, modes)))
    }

    /// Query an user by their osu id.
    pub async fn by_osu_id(osu_id: i64, conn: &Pool) -> Result<Option<Self>> {
        let u = match query_as!(
            raw::OsuUser,
            r#"SELECT
                user_id as "user_id: i64",
                username,
                id as "id: i64",
                preferred_mode as "preferred_mode: u8",
                failures as "failures: u8"
            FROM osu_users WHERE id = ?"#,
            osu_id
        )
        .fetch_optional(conn)
        .await?
        {
            Some(v) => v,
            None => return Ok(None),
        };
        let modes = OsuUserMode::from_user(u.user_id, conn).await?;
        Ok(Some(Self::from_raw(u, modes)))
    }

    /// Query all users.
    pub async fn all(conn: &Pool) -> Result<Vec<Self>> {
        // last_update as "last_update: DateTime",
        let us = query_as!(
            raw::OsuUser,
            r#"SELECT
                user_id as "user_id: i64",
                username,
                id as "id: i64",
                preferred_mode as "preferred_mode: u8",
                failures as "failures: u8"
            FROM osu_users"#,
        )
        .fetch_all(conn)
        .await?;
        let mut modes = OsuUserMode::fetch_all(conn).await?;
        Ok(us
            .into_iter()
            .map(|u| {
                let m = modes.remove(&u.user_id).unwrap_or_default();
                Self::from_raw(u, m)
            })
            .collect())
    }
}

impl OsuUser {
    /// Stores the user.
    pub async fn store(&self, conn: &mut Transaction<'_, Database>) -> Result<bool> {
        let old_user_id = {
            query!(
                r#"SELECT id as "id: i64" FROM osu_users WHERE user_id = ?"#,
                self.user_id
            )
            .fetch_optional(&mut **conn)
            .await?
            .map(|v| v.id)
        };

        if old_user_id.is_some_and(|v| v != self.id) {
            // There's another update that changed the user_id
            return Ok(false);
        }

        query!(
            r#"INSERT
               INTO osu_users(user_id, username, id, preferred_mode, failures)
               VALUES(?, ?, ?, ?, ?)
               ON CONFLICT (user_id) WHERE id = ? DO UPDATE
               SET
                username = excluded.username,
                preferred_mode = excluded.preferred_mode,
                failures = excluded.failures
            "#,
            self.user_id,
            self.username,
            self.id,
            self.preferred_mode,
            self.failures,
            self.user_id,
        )
        .execute(&mut **conn)
        .await?;
        // Store the modes
        query!(
            "DELETE FROM osu_user_mode_stats WHERE user_id = ?",
            self.user_id
        )
        .execute(&mut **conn)
        .await?;
        for (mode, stats) in &self.modes {
            let ts = stats.last_update.timestamp();
            query!(
                "INSERT INTO osu_user_mode_stats (user_id, mode, pp, map_length, map_age, last_update) VALUES (?, ?, ?, ?, ?, ?)",
                self.user_id,
                *mode,
                stats.pp,
                stats.map_length,
                stats.map_age,
                ts,
            )
            .execute(&mut **conn)
            .await?;
        }
        Ok(true)
    }

    pub async fn delete(user_id: i64, conn: impl Executor<'_, Database = Database>) -> Result<()> {
        query!("DELETE FROM osu_users WHERE user_id = ?", user_id)
            .execute(conn)
            .await?;
        Ok(())
    }
}
