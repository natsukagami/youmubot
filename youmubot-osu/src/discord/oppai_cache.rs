use std::{ffi::CString, sync::Arc};
use youmubot_db_sql::{models::osu as models, Pool};
use youmubot_prelude::*;

pub use oppai_rs::Accuracy as OppaiAccuracy;

/// the information collected from a download/Oppai request.
#[derive(Debug)]
pub struct BeatmapContent {
    id: u64,
    content: Arc<CString>,
}

/// the output of "one" oppai run.
#[derive(Clone, Copy, Debug)]
pub struct BeatmapInfo {
    pub objects: u32,
    pub stars: f32,
}

/// Beatmap Info with attached 95/98/99/100% FC pp.
pub type BeatmapInfoWithPP = (BeatmapInfo, [f32; 4]);

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
        oppai.mods(mods.into());
        let objects = oppai.num_objects();
        let stars = oppai.stars();
        Ok(BeatmapInfo { stars, objects })
    }

    pub fn get_possible_pp_with(
        &self,
        mode: Option<oppai_rs::Mode>,
        mods: impl Into<oppai_rs::Mods>,
    ) -> Result<BeatmapInfoWithPP> {
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
        let objects = oppai.num_objects();
        let stars = oppai.stars();
        Ok((BeatmapInfo { stars, objects }, pp))
    }
}

/// A central cache for the beatmaps.
pub struct BeatmapCache {
    client: ratelimit::Ratelimit<reqwest::Client>,
    pool: Pool,
}

impl BeatmapCache {
    /// Create a new cache.
    pub fn new(client: reqwest::Client, pool: Pool) -> Self {
        let client = ratelimit::Ratelimit::new(client, 5, std::time::Duration::from_secs(1));
        BeatmapCache { client, pool }
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
            content: Arc::new(CString::new(content.into_iter().collect::<Vec<_>>())?),
        })
    }

    async fn get_beatmap_db(&self, id: u64) -> Result<Option<BeatmapContent>> {
        Ok(models::CachedBeatmapContent::by_id(id as i64, &self.pool)
            .await?
            .map(|v| BeatmapContent {
                id,
                content: Arc::new(CString::new(v.content).unwrap()),
            }))
    }

    async fn save_beatmap(&self, b: &BeatmapContent) -> Result<()> {
        let mut bc = models::CachedBeatmapContent {
            beatmap_id: b.id as i64,
            cached_at: chrono::Utc::now(),
            content: b.content.as_ref().clone().into_bytes(),
        };
        bc.store(&self.pool).await?;
        Ok(())
    }

    /// Get a beatmap from the cache.
    pub async fn get_beatmap(&self, id: u64) -> Result<BeatmapContent> {
        match self.get_beatmap_db(id).await? {
            Some(v) => Ok(v),
            None => {
                let m = self.download_beatmap(id).await?;
                self.save_beatmap(&m).await?;
                Ok(m)
            }
        }
    }
}

impl TypeMapKey for BeatmapCache {
    type Value = BeatmapCache;
}
