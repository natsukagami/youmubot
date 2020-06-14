use crate::{
    models::{ApprovalStatus, Beatmap, Mode},
    Client,
};
use dashmap::DashMap;
use serenity::framework::standard::CommandError;
use std::sync::Arc;
use youmubot_prelude::TypeMapKey;

/// BeatmapMetaCache intercepts beatmap-by-id requests and caches them for later recalling.
/// Does not cache non-Ranked beatmaps.
#[derive(Clone, Debug)]
pub struct BeatmapMetaCache {
    client: Client,
    cache: Arc<DashMap<(u64, Mode), Beatmap>>,
}

impl TypeMapKey for BeatmapMetaCache {
    type Value = BeatmapMetaCache;
}

impl BeatmapMetaCache {
    /// Create a new beatmap cache.
    pub fn new(client: Client) -> Self {
        BeatmapMetaCache {
            client,
            cache: Arc::new(DashMap::new()),
        }
    }
    fn insert_if_possible(&self, id: u64, mode: Option<Mode>) -> Result<Beatmap, CommandError> {
        let beatmap = self
            .client
            .beatmaps(crate::BeatmapRequestKind::Beatmap(id), |f| {
                if let Some(mode) = mode {
                    f.mode(mode, true);
                }
                f
            })
            .and_then(|v| {
                v.into_iter()
                    .next()
                    .ok_or(CommandError::from("beatmap not found"))
            })?;
        if let ApprovalStatus::Ranked(_) = beatmap.approval {
            self.cache.insert((id, beatmap.mode), beatmap.clone());
        };
        Ok(beatmap)
    }
    /// Get the given beatmap
    pub fn get_beatmap(&self, id: u64, mode: Mode) -> Result<Beatmap, CommandError> {
        self.cache
            .get(&(id, mode))
            .map(|b| Ok(b.clone()))
            .unwrap_or_else(|| self.insert_if_possible(id, Some(mode)))
    }

    /// Get a beatmap without a mode...
    pub fn get_beatmap_default(&self, id: u64) -> Result<Beatmap, CommandError> {
        (&[Mode::Std, Mode::Taiko, Mode::Catch, Mode::Mania])
            .iter()
            .filter_map(|&mode| {
                self.cache
                    .get(&(id, mode))
                    .filter(|b| b.mode == mode)
                    .map(|b| Ok(b.clone()))
            })
            .next()
            .unwrap_or_else(|| self.insert_if_possible(id, None))
    }
}
