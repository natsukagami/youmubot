use crate::mods::Mods;
use rosu_pp::{Beatmap, BeatmapExt};
use std::sync::Arc;
use youmubot_db_sql::{models::osu as models, Pool};
use youmubot_prelude::*;

/// the information collected from a download/Oppai request.
#[derive(Debug)]
pub struct BeatmapContent {
    id: u64,
    content: Arc<Beatmap>,
}

/// the output of "one" oppai run.
#[derive(Clone, Copy, Debug)]
pub struct BeatmapInfo {
    pub objects: usize,
    pub stars: f64,
}

#[derive(Clone, Copy, Debug)]
pub enum Accuracy {
    ByCount(u64, u64, u64, u64), // 300 / 100 / 50 / misses
    #[allow(dead_code)]
    ByValue(f64, u64),
}

impl Into<f64> for Accuracy {
    fn into(self) -> f64 {
        match self {
            Accuracy::ByValue(v, _) => v,
            Accuracy::ByCount(n300, n100, n50, nmiss) => {
                100.0 * ((6 * n300 + 2 * n100 + n50) as f64)
                    / ((6 * (n300 + n100 + n50 + nmiss)) as f64)
            }
        }
    }
}

impl Accuracy {
    pub fn misses(&self) -> usize {
        (match self {
            Accuracy::ByCount(_, _, _, nmiss) => *nmiss,
            Accuracy::ByValue(_, nmiss) => *nmiss,
        }) as usize
    }
}

/// Beatmap Info with attached 95/98/99/100% FC pp.
pub type BeatmapInfoWithPP = (BeatmapInfo, [f64; 4]);

impl BeatmapContent {
    /// Get pp given the combo and accuracy.
    pub fn get_pp_from(&self, combo: Option<usize>, accuracy: Accuracy, mods: Mods) -> Result<f64> {
        let bm = self.content.as_ref();
        let mut rosu = rosu_pp::OsuPP::new(bm).mods(mods.bits() as u32);
        if let Some(combo) = combo {
            rosu = rosu.combo(combo);
        }
        if let Accuracy::ByCount(n300, n100, n50, _) = accuracy {
            rosu = rosu
                .n300(n300 as usize)
                .n100(n100 as usize)
                .n50(n50 as usize);
        }
        Ok(rosu
            .misses(accuracy.misses())
            .accuracy(accuracy.into())
            .calculate()
            .pp)
    }

    /// Get info given mods.
    pub fn get_info_with(&self, mods: Mods) -> Result<BeatmapInfo> {
        let stars = self.content.stars(mods.bits() as u32, None);
        Ok(BeatmapInfo {
            objects: stars.max_combo().unwrap_or(0),
            stars: stars.stars(),
        })
    }

    pub fn get_possible_pp_with(&self, mods: Mods) -> Result<BeatmapInfoWithPP> {
        let rosu = || self.content.pp().mods(mods.bits() as u32);
        let pp95 = rosu().accuracy(95.0).calculate();
        let pp = [
            pp95.pp(),
            rosu()
                .attributes(pp95.clone())
                .accuracy(98.0)
                .calculate()
                .pp(),
            rosu()
                .attributes(pp95.clone())
                .accuracy(99.0)
                .calculate()
                .pp(),
            rosu()
                .attributes(pp95.clone())
                .accuracy(100.0)
                .calculate()
                .pp(),
        ];
        let objects = pp95.difficulty_attributes().max_combo().unwrap_or(0);
        let stars = pp95.difficulty_attributes().stars();
        Ok((BeatmapInfo { objects, stars }, pp))
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
        let bm = BeatmapContent {
            id,
            content: Arc::new(Beatmap::parse(content.as_ref())?),
        };

        let mut bc = models::CachedBeatmapContent {
            beatmap_id: id as i64,
            cached_at: chrono::Utc::now(),
            content: content.as_ref().to_owned(),
        };
        bc.store(&self.pool).await?;
        Ok(bm)
    }

    async fn get_beatmap_db(&self, id: u64) -> Result<Option<BeatmapContent>> {
        Ok(models::CachedBeatmapContent::by_id(id as i64, &self.pool)
            .await?
            .map(|v| {
                Ok(BeatmapContent {
                    id,
                    content: Arc::new(Beatmap::parse(&v.content[..])?),
                }) as Result<_>
            })
            .transpose()?)
    }

    /// Get a beatmap from the cache.
    pub async fn get_beatmap(&self, id: u64) -> Result<BeatmapContent> {
        match self.get_beatmap_db(id).await? {
            Some(v) => Ok(v),
            None => self.download_beatmap(id).await,
        }
    }
}

impl TypeMapKey for BeatmapCache {
    type Value = BeatmapCache;
}
