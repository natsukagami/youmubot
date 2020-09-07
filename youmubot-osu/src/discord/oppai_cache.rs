use std::ffi::CString;
use youmubot_prelude::*;

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
        accuracy: f32,
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
#[derive(Debug)]
pub struct BeatmapCache {
    client: reqwest::Client,
    cache: dashmap::DashMap<u64, BeatmapContent>,
}

impl BeatmapCache {
    /// Create a new cache.
    pub fn new(client: reqwest::Client) -> Self {
        BeatmapCache {
            client,
            cache: dashmap::DashMap::new(),
        }
    }

    async fn download_beatmap(&self, id: u64) -> Result<BeatmapContent> {
        let content = self
            .client
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
    pub async fn get_beatmap<'a>(
        &'a self,
        id: u64,
    ) -> Result<impl std::ops::Deref<Target = BeatmapContent> + 'a> {
        if !self.cache.contains_key(&id) {
            self.cache.insert(id, self.download_beatmap(id).await?);
        }
        Ok(self.cache.get(&id).unwrap())
    }
}

impl TypeMapKey for BeatmapCache {
    type Value = BeatmapCache;
}
