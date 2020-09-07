use crate::{
    models::{ApprovalStatus, Beatmap, Mode},
    Client,
};
use dashmap::DashMap;
use std::sync::Arc;
use youmubot_prelude::*;

/// BeatmapMetaCache intercepts beatmap-by-id requests and caches them for later recalling.
/// Does not cache non-Ranked beatmaps.
pub struct BeatmapMetaCache {
    client: Arc<Client>,
    cache: DashMap<(u64, Mode), Beatmap>,
}

impl TypeMapKey for BeatmapMetaCache {
    type Value = BeatmapMetaCache;
}

impl BeatmapMetaCache {
    /// Create a new beatmap cache.
    pub fn new(client: Arc<Client>) -> Self {
        BeatmapMetaCache {
            client,
            cache: DashMap::new(),
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
            .and_then(|v| v.into_iter().next().ok_or(Error::msg("beatmap not found")))?;
        if let ApprovalStatus::Ranked(_) = beatmap.approval {
            self.cache.insert((id, beatmap.mode), beatmap.clone());
        };
        Ok(beatmap)
    }
    /// Get the given beatmap
    pub async fn get_beatmap(&self, id: u64, mode: Mode) -> Result<Beatmap> {
        match self.cache.get(&(id, mode)).map(|v| v.clone()) {
            Some(v) => Ok(v),
            None => self.insert_if_possible(id, Some(mode)).await,
        }
    }

    /// Get a beatmap without a mode...
    pub async fn get_beatmap_default(&self, id: u64) -> Result<Beatmap> {
        Ok(
            match (&[Mode::Std, Mode::Taiko, Mode::Catch, Mode::Mania])
                .iter()
                .filter_map(|&mode| {
                    self.cache
                        .get(&(id, mode))
                        .filter(|b| b.mode == mode)
                        .map(|b| b.clone())
                })
                .next()
            {
                Some(v) => v,
                None => self.insert_if_possible(id, None).await?,
            },
        )
    }
}
