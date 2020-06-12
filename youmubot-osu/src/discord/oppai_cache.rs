use serenity::framework::standard::CommandError;
use std::{ffi::CString, sync::Arc};
use youmubot_prelude::TypeMapKey;

/// the information collected from a download/Oppai request.
#[derive(Clone, Debug)]
pub struct BeatmapContent {
    id: u64,
    content: Arc<CString>,
}

/// the output of "one" oppai run.
#[derive(Clone, Copy, Debug)]
pub struct BeatmapInfo {
    pub stars: f32,
    pub pp: [f32; 4], // 95, 98, 99, 100
}

impl BeatmapContent {
    /// Get pp given the combo and accuracy.
    pub fn get_pp_from(
        &self,
        combo: oppai_rs::Combo,
        accuracy: f32,
        mode: Option<oppai_rs::Mode>,
        mods: impl Into<oppai_rs::Mods>,
    ) -> Result<f32, CommandError> {
        let mut oppai = oppai_rs::Oppai::new_from_content(&self.content[..])?;
        oppai.combo(combo)?.accuracy(accuracy)?.mods(mods.into());
        if let Some(mode) = mode {
            oppai.mode(mode)?;
        }
        Ok(oppai.pp())
    }

    /// Get info given mods.
    pub fn get_info_with(
        &self,
        mode: Option<oppai_rs::Mode>,
        mods: impl Into<oppai_rs::Mods>,
    ) -> Result<BeatmapInfo, CommandError> {
        let mut oppai = oppai_rs::Oppai::new_from_content(&self.content[..])?;
        if let Some(mode) = mode {
            oppai.mode(mode)?;
        }
        oppai.mods(mods.into()).combo(oppai_rs::Combo::PERFECT)?;
        let pp = [
            oppai.accuracy(95.0)?.pp(),
            oppai.accuracy(98.0)?.pp(),
            oppai.accuracy(99.0)?.pp(),
            oppai.accuracy(100.0)?.pp(),
        ];
        let stars = oppai.stars();
        Ok(BeatmapInfo { stars, pp })
    }
}

/// A central cache for the beatmaps.
#[derive(Clone, Debug)]
pub struct BeatmapCache {
    client: reqwest::blocking::Client,
    cache: Arc<dashmap::DashMap<u64, BeatmapContent>>,
}

impl BeatmapCache {
    /// Create a new cache.
    pub fn new(client: reqwest::blocking::Client) -> Self {
        BeatmapCache {
            client,
            cache: Arc::new(dashmap::DashMap::new()),
        }
    }

    fn download_beatmap(&self, id: u64) -> Result<BeatmapContent, CommandError> {
        let content = self
            .client
            .get(&format!("https://osu.ppy.sh/osu/{}", id))
            .send()?
            .bytes()?;
        Ok(BeatmapContent {
            id,
            content: Arc::new(CString::new(content.into_iter().collect::<Vec<_>>())?),
        })
    }

    /// Get a beatmap from the cache.
    pub fn get_beatmap(&self, id: u64) -> Result<BeatmapContent, CommandError> {
        self.cache
            .entry(id)
            .or_try_insert_with(|| self.download_beatmap(id))
            .map(|v| v.clone())
    }
}

impl TypeMapKey for BeatmapCache {
    type Value = BeatmapCache;
}
