use crate::{
    models::{ApprovalStatus, Beatmap, Mode},
    Client,
};
use std::sync::Arc;
use youmubot_db_sql::{models::osu as models, Pool};
use youmubot_prelude::*;

/// BeatmapMetaCache intercepts beatmap-by-id requests and caches them for later recalling.
/// Does not cache non-Ranked beatmaps.
#[derive(Clone)]
pub struct BeatmapMetaCache {
    client: Arc<Client>,
    pool: Pool,
}

impl std::fmt::Debug for BeatmapMetaCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<BeatmapMetaCache>")
    }
}

impl TypeMapKey for BeatmapMetaCache {
    type Value = BeatmapMetaCache;
}

impl BeatmapMetaCache {
    /// Create a new beatmap cache.
    pub fn new(client: Arc<Client>, pool: Pool) -> Self {
        BeatmapMetaCache { client, pool }
    }

    /// Clean the cache.
    pub async fn clear(&self) -> Result<()> {
        models::CachedBeatmap::clear_all(&self.pool).await?;
        Ok(())
    }

    #[allow(clippy::wrong_self_convention)]
    fn to_cached_beatmap(beatmap: &Beatmap, mode: Option<Mode>) -> models::CachedBeatmap {
        models::CachedBeatmap {
            beatmap_id: beatmap.beatmap_id as i64,
            mode: mode.unwrap_or(beatmap.mode) as u8,
            cached_at: chrono::Utc::now(),
            beatmap: bincode::serialize(&beatmap).unwrap(),
        }
    }

    async fn insert_if_possible(&self, id: u64, mode: Option<Mode>) -> Result<Beatmap> {
        let beatmap = self
            .client
            .beatmaps(crate::BeatmapRequestKind::Beatmap(id), |f| {
                if let Some(mode) = mode {
                    f.mode(mode, true);
                }
                f
            })
            .await
            .and_then(|v| {
                v.into_iter()
                    .next()
                    .ok_or_else(|| Error::msg("beatmap not found"))
            })?;
        if let ApprovalStatus::Ranked(_) = beatmap.approval {
            let mut c = Self::to_cached_beatmap(&beatmap, mode);
            c.store(&self.pool).await.pls_ok();
        };
        Ok(beatmap)
    }

    async fn get_beatmap_db(&self, id: u64, mode: Mode) -> Result<Option<Beatmap>> {
        Ok(
            models::CachedBeatmap::by_id(id as i64, mode as u8, &self.pool)
                .await?
                .map(|v| bincode::deserialize(&v.beatmap[..]).unwrap()),
        )
    }

    /// Get the given beatmap
    pub async fn get_beatmap(&self, id: u64, mode: Mode) -> Result<Beatmap> {
        match self.get_beatmap_db(id, mode).await? {
            Some(v) => Ok(v),
            None => self.insert_if_possible(id, Some(mode)).await,
        }
    }

    /// Get a beatmap without a mode...
    pub async fn get_beatmap_default(&self, id: u64) -> Result<Beatmap> {
        for mode in [Mode::Std, Mode::Taiko, Mode::Catch, Mode::Mania].into_iter() {
            if let Ok(Some(bm)) = self.get_beatmap_db(id, mode).await {
                if bm.mode == mode {
                    return Ok(bm);
                }
            }
        }

        self.insert_if_possible(id, None).await
    }

    /// Get a beatmapset from its ID.
    pub async fn get_beatmapset(&self, id: u64) -> Result<Vec<Beatmap>> {
        let bms = models::CachedBeatmap::by_beatmapset(id as i64, &self.pool).await?;
        if !bms.is_empty() {
            return Ok(bms
                .into_iter()
                .map(|v| bincode::deserialize(&v.beatmap[..]).unwrap())
                .collect());
        }
        let mut beatmaps = self
            .client
            .beatmaps(crate::BeatmapRequestKind::Beatmapset(id), |f| f)
            .await?;
        if beatmaps.is_empty() {
            return Err(Error::msg("beatmapset not found"));
        }
        beatmaps.sort_by_key(|b| (b.mode as u8, (b.difficulty.stars * 1000.0) as u64)); // Cast so that Ord is maintained
        if let ApprovalStatus::Ranked(_) = &beatmaps[0].approval {
            // Save each beatmap.
            let mut t = self.pool.begin().await?;
            for b in &beatmaps {
                let mut b = Self::to_cached_beatmap(b, None);
                b.store(&mut *t).await?;
                // Save the beatmapset mapping.
                b.link_beatmapset(id as i64, &mut *t).await?;
            }
            t.commit().await?;
        }
        Ok(beatmaps)
    }
}
