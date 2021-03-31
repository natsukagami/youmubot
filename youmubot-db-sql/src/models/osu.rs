use crate::models::*;

pub struct LastBeatmap {
    pub channel_id: i64,
    pub beatmap: Vec<u8>,
    pub mode: u8,
}

impl LastBeatmap {
    /// Get a [`LastBeatmap`] by the channel id.
    pub async fn by_channel_id(
        id: i64,
        conn: impl Executor<'_, Database = Database>,
    ) -> Result<Option<LastBeatmap>> {
        let m = query_as!(
            LastBeatmap,
            r#"SELECT
                    channel_id as "channel_id: i64",
                    beatmap,
                    mode as "mode: u8"
                FROM osu_last_beatmaps
                WHERE channel_id = ?"#,
            id
        )
        .fetch_optional(conn)
        .await?;
        Ok(m)
    }
}

impl LastBeatmap {
    /// Store the value.
    pub async fn store(&self, conn: impl Executor<'_, Database = Database>) -> Result<()> {
        query!(
            r#"INSERT INTO
                  osu_last_beatmaps (channel_id, beatmap, mode)
               VALUES
                  (?, ?, ?)
               ON CONFLICT (channel_id) DO UPDATE
                  SET
                    beatmap = excluded.beatmap,
                    mode = excluded.mode"#,
            self.channel_id,
            self.beatmap,
            self.mode,
        )
        .execute(conn)
        .await?;
        Ok(())
    }
}

pub struct UserBestScore {
    pub beatmap_id: i64,
    pub mode: u8,
    pub user_id: i64,
    pub mods: u32,

    pub cached_at: DateTime,
    /// To be deserialized by `bincode`
    pub score: Vec<u8>,
}

impl UserBestScore {
    /// Get a list of scores by the given map and user.
    pub async fn by_map_and_user(
        beatmap: i64,
        mode: u8,
        user: i64,
        conn: impl Executor<'_, Database = Database>,
    ) -> Result<Vec<Self>> {
        query_as!(
            UserBestScore,
            r#"SELECT
                beatmap_id as "beatmap_id: i64",
                mode as "mode: u8",
                user_id as "user_id: i64",
                mods as "mods: u32",
                cached_at as "cached_at: DateTime",
                score as "score: Vec<u8>"
            FROM osu_user_best_scores
            WHERE
                beatmap_id = ?
                AND mode = ?
                AND user_id = ?"#,
            beatmap,
            mode,
            user
        )
        .fetch_all(conn)
        .await
        .map_err(Error::from)
    }
    /// Get a list of scores by the given map.
    pub async fn by_map(
        beatmap: i64,
        mode: u8,
        conn: impl Executor<'_, Database = Database>,
    ) -> Result<Vec<Self>> {
        query_as!(
            UserBestScore,
            r#"SELECT
                beatmap_id as "beatmap_id: i64",
                mode as "mode: u8",
                user_id as "user_id: i64",
                mods as "mods: u32",
                cached_at as "cached_at: DateTime",
                score as "score: Vec<u8>"
            FROM osu_user_best_scores
            WHERE
                beatmap_id = ?
                AND mode = ?"#,
            beatmap,
            mode
        )
        .fetch_all(conn)
        .await
        .map_err(Error::from)
    }
}

impl UserBestScore {
    pub async fn store(&mut self, conn: impl Executor<'_, Database = Database>) -> Result<()> {
        self.cached_at = chrono::Utc::now();
        query!(
            r#"
                INSERT INTO
                    osu_user_best_scores (beatmap_id, mode, user_id, mods, cached_at, score)
                VALUES
                    (?, ?, ?, ?, ?, ?)
                ON CONFLICT (beatmap_id, mode, user_id, mods)
                DO UPDATE
                    SET
                        cached_at = excluded.cached_at,
                        score = excluded.score
            "#,
            self.beatmap_id,
            self.mode,
            self.user_id,
            self.mods,
            self.cached_at,
            self.score
        )
        .execute(conn)
        .await?;
        Ok(())
    }
}

pub struct CachedBeatmap {
    pub beatmap_id: i64,
    pub mode: u8,
    pub cached_at: DateTime,
    pub beatmap: Vec<u8>,
}

impl CachedBeatmap {
    /// Get a cached beatmap by its id.
    pub async fn by_id(
        id: i64,
        mode: u8,
        conn: impl Executor<'_, Database = Database>,
    ) -> Result<Option<Self>> {
        query_as!(
            Self,
            r#"SELECT
                beatmap_id as "beatmap_id: i64",
                mode as "mode: u8",
                cached_at as "cached_at: DateTime",
                beatmap as "beatmap: Vec<u8>"
            FROM osu_cached_beatmaps
            WHERE
                beatmap_id = ?
                AND mode = ?
                "#,
            id,
            mode
        )
        .fetch_optional(conn)
        .await
        .map_err(Error::from)
    }
}

impl CachedBeatmap {
    pub async fn store(&mut self, conn: impl Executor<'_, Database = Database>) -> Result<()> {
        self.cached_at = chrono::Utc::now();
        query!(
            r#"
                INSERT INTO
                    osu_cached_beatmaps (beatmap_id, mode, cached_at, beatmap)
                VALUES
                    (?, ?, ?, ?)
                ON CONFLICT (beatmap_id, mode)
                DO UPDATE
                    SET
                        cached_at = excluded.cached_at,
                        beatmap = excluded.beatmap
            "#,
            self.beatmap_id,
            self.mode,
            self.cached_at,
            self.beatmap
        )
        .execute(conn)
        .await?;
        Ok(())
    }
}

pub struct CachedBeatmapContent {
    pub beatmap_id: i64,
    pub cached_at: DateTime,
    pub content: Vec<u8>,
}

impl CachedBeatmapContent {
    /// Get a cached beatmap by its id.
    pub async fn by_id(
        id: i64,
        conn: impl Executor<'_, Database = Database>,
    ) -> Result<Option<Self>> {
        query_as!(
            Self,
            r#"SELECT
                beatmap_id as "beatmap_id: i64",
                cached_at as "cached_at: DateTime",
                content as "content: Vec<u8>"
            FROM osu_cached_beatmap_contents
            WHERE
                beatmap_id = ? "#,
            id,
        )
        .fetch_optional(conn)
        .await
        .map_err(Error::from)
    }
}

impl CachedBeatmapContent {
    pub async fn store(&mut self, conn: impl Executor<'_, Database = Database>) -> Result<()> {
        self.cached_at = chrono::Utc::now();
        query!(
            r#"
                INSERT INTO
                    osu_cached_beatmap_contents (beatmap_id, cached_at, content)
                VALUES
                    (?, ?, ?)
                ON CONFLICT (beatmap_id)
                DO UPDATE
                    SET
                        cached_at = excluded.cached_at,
                        content = excluded.content
            "#,
            self.beatmap_id,
            self.cached_at,
            self.content
        )
        .execute(conn)
        .await?;
        Ok(())
    }
}
