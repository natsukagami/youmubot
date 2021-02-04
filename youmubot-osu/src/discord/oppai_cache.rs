use std::{ffi::CString, sync::Arc};
use youmubot_prelude::*;

pub use oppai_rs::Accuracy as OppaiAccuracy;

/// the information collected from a download/Oppai request.
#[derive(Debug)]
pub struct BeatmapContent {
    id: u64,
    content: CString,
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
        accuracy: impl Into<OppaiAccuracy>,
        mode: Option<oppai_rs::Mode>,
        mods: impl Into<oppai_rs::Mods>,
    ) -> Result<f32> {
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
    ) -> Result<BeatmapInfo> {
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
pub struct BeatmapCache {
    client: ratelimit::Ratelimit<reqwest::Client>,
    cache: dashmap::DashMap<u64, Arc<BeatmapContent>>,
}

impl BeatmapCache {
    /// Create a new cache.
    pub fn new(client: reqwest::Client) -> Self {
        let client = ratelimit::Ratelimit::new(client, 5, std::time::Duration::from_secs(1));
        BeatmapCache {
            client,
            cache: dashmap::DashMap::new(),
        }
    }

    async fn download_beatmap(&self, id: u64) -> Result<BeatmapContent> {
        let content = self
            .client
            .borrow()
            .await?
            .get(&format!("https://osu.ppy.sh/osu/{}", id))
            .send()
            .await?
            .bytes()
            .await?;
        Ok(BeatmapContent {
            id,
            content: CString::new(content.into_iter().collect::<Vec<_>>())?,
        })
    }

    /// Get a beatmap from the cache.
    pub async fn get_beatmap(
        &self,
        id: u64,
    ) -> Result<impl std::ops::Deref<Target = BeatmapContent>> {
        if !self.cache.contains_key(&id) {
            self.cache
                .insert(id, Arc::new(self.download_beatmap(id).await?));
        }
        Ok(self.cache.get(&id).unwrap().clone())
    }
}

impl TypeMapKey for BeatmapCache {
    type Value = BeatmapCache;
}
