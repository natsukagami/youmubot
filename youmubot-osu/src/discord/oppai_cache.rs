use std::io::Read;
use std::sync::Arc;

use osuparse::MetadataSection;
use rosu_pp::any::DifficultyAttributes;
use rosu_pp::Beatmap;

use youmubot_db_sql::{models::osu as models, Pool};
use youmubot_prelude::*;

use crate::{models::Mode, mods::Mods};

/// the information collected from a download/Oppai request.
#[derive(Debug)]
pub struct BeatmapContent {
    pub metadata: MetadataSection,
    pub content: Arc<Beatmap>,
}

/// the output of "one" oppai run.
#[derive(Clone, Copy, Debug)]
pub struct BeatmapInfo {
    pub objects: usize,
    pub max_combo: usize,
    pub stars: f64,
}

impl BeatmapInfo {
    fn extract(beatmap: &Beatmap, attrs: DifficultyAttributes) -> Self {
        BeatmapInfo {
            objects: beatmap.hit_objects.len(),
            max_combo: attrs.max_combo() as usize,
            stars: attrs.stars(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Accuracy {
    ByCount(u64, u64, u64, u64),
    // 300 / 100 / 50 / misses
    #[allow(dead_code)]
    ByValue(f64, u64),
}

impl From<Accuracy> for f64 {
    fn from(val: Accuracy) -> Self {
        match val {
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
    pub fn get_pp_from(
        &self,
        mode: Mode,
        combo: Option<usize>,
        accuracy: Accuracy,
        mods: Mods,
    ) -> Result<f64> {
        let mut perf = self
            .content
            .performance()
            .mode_or_ignore(mode.into())
            .accuracy(accuracy.into())
            .misses(accuracy.misses() as u32)
            .mods(mods.bits() as u32);
        if let Some(combo) = combo {
            perf = perf.combo(combo as u32);
        }
        let attrs = perf.calculate();
        Ok(attrs.pp())
    }

    /// Get info given mods.
    pub fn get_info_with(&self, mode: Mode, mods: Mods) -> Result<BeatmapInfo> {
        let attrs = self
            .content
            .performance()
            .mode_or_ignore(mode.into())
            .mods(mods.bits() as u32)
            .calculate();
        Ok(BeatmapInfo {
            objects: self.content.hit_objects.len(),
            max_combo: attrs.max_combo() as usize,
            stars: attrs.stars(),
        })
    }

    pub fn get_possible_pp_with(&self, mode: Mode, mods: Mods) -> Result<BeatmapInfoWithPP> {
        let pp: [f64; 4] = [
            self.get_pp_from(mode, None, Accuracy::ByValue(95.0, 0), mods)?,
            self.get_pp_from(mode, None, Accuracy::ByValue(98.0, 0), mods)?,
            self.get_pp_from(mode, None, Accuracy::ByValue(99.0, 0), mods)?,
            self.get_pp_from(mode, None, Accuracy::ByValue(100.0, 0), mods)?,
        ];
        Ok((self.get_info_with(mode, mods)?, pp))
    }
}

impl From<Mode> for rosu_pp::model::mode::GameMode {
    fn from(value: Mode) -> Self {
        use rosu_pp::model::mode::GameMode;
        match value {
            Mode::Std => GameMode::Osu,
            Mode::Taiko => GameMode::Taiko,
            Mode::Catch => GameMode::Catch,
            Mode::Mania => GameMode::Mania,
        }
    }
}

/// A central cache for the beatmaps.
#[derive(Debug, Clone)]
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

    /// Clean the cache.
    pub async fn clear(&self) -> Result<()> {
        models::CachedBeatmapContent::clear_all(&self.pool).await?;
        Ok(())
    }

    fn parse_beatmap(content: impl AsRef<str>) -> Result<BeatmapContent> {
        let content = content.as_ref();
        let metadata = osuparse::parse_beatmap(content)
            .map_err(|e| Error::msg(format!("Cannot parse metadata: {:?}", e)))?
            .metadata;
        Ok(BeatmapContent {
            metadata,
            content: Arc::new(Beatmap::from_bytes(content.as_bytes())?),
        })
    }

    /// Downloads the given osz and try to parse every osu file in there (limited to <1mb files)
    pub async fn download_osz_from_url(
        &self,
        url: impl reqwest::IntoUrl,
    ) -> Result<Vec<BeatmapContent>> {
        let osz = self
            .client
            .borrow()
            .await?
            .get(url)
            .send()
            .await?
            .bytes()
            .await?;

        let mut osz = zip::read::ZipArchive::new(std::io::Cursor::new(osz.as_ref()))?;
        let osu_files = osz.file_names().map(|v| v.to_owned()).collect::<Vec<_>>();
        let osu_files = osu_files
            .into_iter()
            .filter(|n| n.ends_with(".osu"))
            .filter_map(|v| {
                let mut v = osz.by_name(&v[..]).ok()?;
                if v.size() > 1024 * 1024
                /*1mb*/
                {
                    return None;
                };
                let mut content = String::new();
                v.read_to_string(&mut content).pls_ok()?;
                Self::parse_beatmap(content).pls_ok()
            })
            .collect::<Vec<_>>();
        Ok(osu_files)
    }

    /// Downloads the beatmap from an URL and returns it.
    /// Does not deal with any caching.
    pub async fn download_beatmap_from_url(
        &self,
        url: impl reqwest::IntoUrl,
    ) -> Result<(BeatmapContent, String)> {
        let content = self
            .client
            .borrow()
            .await?
            .get(url)
            .send()
            .await?
            .text()
            .await?;
        let bm = Self::parse_beatmap(&content)?;
        Ok((bm, content))
    }

    async fn download_beatmap(&self, id: u64) -> Result<BeatmapContent> {
        let (bm, content) = self
            .download_beatmap_from_url(&format!("https://osu.ppy.sh/osu/{}", id))
            .await?;

        let mut bc = models::CachedBeatmapContent {
            beatmap_id: id as i64,
            cached_at: chrono::Utc::now(),
            content: content.into_bytes(),
        };
        bc.store(&self.pool).await?;
        Ok(bm)
    }

    async fn get_beatmap_db(&self, id: u64) -> Result<Option<BeatmapContent>> {
        models::CachedBeatmapContent::by_id(id as i64, &self.pool)
            .await?
            .map(|v| Self::parse_beatmap(String::from_utf8(v.content)?))
            .transpose()
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
