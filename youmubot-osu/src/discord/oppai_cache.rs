use std::io::Read;
use std::sync::Arc;

use osuparse::MetadataSection;
use rosu_pp::catch::CatchDifficultyAttributes;
use rosu_pp::mania::ManiaDifficultyAttributes;
use rosu_pp::osu::OsuDifficultyAttributes;
use rosu_pp::taiko::TaikoDifficultyAttributes;
use rosu_pp::{AttributeProvider, Beatmap, CatchPP, DifficultyAttributes, ManiaPP, OsuPP, TaikoPP};

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
            max_combo: attrs.max_combo(),
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

trait PPCalc<'a>: Sized {
    type Attrs: rosu_pp::AttributeProvider + Clone;

    fn new(beatmap: &'a Beatmap) -> Self;
    fn mods(self, mods: u32) -> Self;
    fn attributes(self, attrs: Self::Attrs) -> Self;

    /* For pp calculation */
    fn combo(self, combo: usize) -> Self;
    fn accuracy(self, accuracy: f64) -> Self;
    fn misses(self, misses: usize) -> Self;
    fn get_pp(self) -> f64;

    /* For difficulty calculation */
    fn get_attrs(self) -> Self::Attrs;

    fn combo_opt(self, combo: Option<usize>) -> Self {
        match combo {
            Some(c) => self.combo(c),
            None => self,
        }
    }
    fn accuracy_from(self, accuracy: Accuracy) -> Self {
        self.misses(accuracy.misses()).accuracy(accuracy.into())
    }

    fn map_attributes(beatmap: &'a Beatmap, mods: Mods) -> Self::Attrs {
        Self::new(beatmap).mods(mods.bits() as u32).get_attrs()
    }
    fn map_pp(beatmap: &'a Beatmap, mods: Mods, combo: Option<usize>, accuracy: Accuracy) -> f64 {
        Self::new(beatmap)
            .mods(mods.bits() as u32)
            .combo_opt(combo)
            .accuracy_from(accuracy)
            .get_pp()
    }
    fn map_info(beatmap: &'a Beatmap, mods: Mods) -> BeatmapInfo {
        let attrs = Self::map_attributes(beatmap, mods).attributes();
        BeatmapInfo::extract(beatmap, attrs)
    }

    fn map_info_with_pp(beatmap: &'a Beatmap, mods: Mods) -> BeatmapInfoWithPP {
        let attrs = Self::map_attributes(beatmap, mods);
        let nw = || {
            Self::new(beatmap)
                .mods(mods.bits() as u32)
                .attributes(attrs.clone())
        };
        let pps = [
            nw().accuracy_from(Accuracy::ByValue(95.0, 0)).get_pp(),
            nw().accuracy_from(Accuracy::ByValue(98.0, 0)).get_pp(),
            nw().accuracy_from(Accuracy::ByValue(99.0, 0)).get_pp(),
            nw().accuracy_from(Accuracy::ByValue(100.0, 0)).get_pp(),
        ];
        let info = BeatmapInfo::extract(beatmap, attrs.attributes());
        (info, pps)
    }
}

impl<'a> PPCalc<'a> for OsuPP<'a> {
    type Attrs = OsuDifficultyAttributes;

    fn new(beatmap: &'a Beatmap) -> Self {
        Self::new(beatmap)
    }
    fn mods(self, mods: u32) -> Self {
        self.mods(mods)
    }

    fn attributes(self, attrs: Self::Attrs) -> Self {
        self.attributes(attrs)
    }

    fn combo(self, combo: usize) -> Self {
        self.combo(combo)
    }

    fn accuracy(self, accuracy: f64) -> Self {
        self.accuracy(accuracy)
    }

    fn misses(self, misses: usize) -> Self {
        self.n_misses(misses)
    }

    fn get_pp(self) -> f64 {
        self.calculate().pp()
    }

    fn get_attrs(self) -> Self::Attrs {
        self.calculate().difficulty
    }
}

impl<'a> PPCalc<'a> for TaikoPP<'a> {
    type Attrs = TaikoDifficultyAttributes;

    fn new(beatmap: &'a Beatmap) -> Self {
        Self::new(beatmap)
    }
    fn mods(self, mods: u32) -> Self {
        self.mods(mods)
    }

    fn attributes(self, attrs: Self::Attrs) -> Self {
        self.attributes(attrs)
    }

    fn combo(self, combo: usize) -> Self {
        self.combo(combo)
    }

    fn accuracy(self, accuracy: f64) -> Self {
        self.accuracy(accuracy)
    }

    fn misses(self, misses: usize) -> Self {
        self.n_misses(misses)
    }

    fn get_pp(self) -> f64 {
        self.calculate().pp()
    }

    fn get_attrs(self) -> Self::Attrs {
        self.calculate().difficulty
    }
}

impl<'a> PPCalc<'a> for CatchPP<'a> {
    type Attrs = CatchDifficultyAttributes;

    fn new(beatmap: &'a Beatmap) -> Self {
        Self::new(beatmap)
    }
    fn mods(self, mods: u32) -> Self {
        self.mods(mods)
    }

    fn attributes(self, attrs: Self::Attrs) -> Self {
        self.attributes(attrs)
    }

    fn combo(self, combo: usize) -> Self {
        self.combo(combo)
    }

    fn accuracy(self, accuracy: f64) -> Self {
        self.accuracy(accuracy)
    }

    fn misses(self, misses: usize) -> Self {
        self.misses(misses)
    }

    fn get_pp(self) -> f64 {
        self.calculate().pp()
    }

    fn get_attrs(self) -> Self::Attrs {
        self.calculate().difficulty
    }
}

impl<'a> PPCalc<'a> for ManiaPP<'a> {
    type Attrs = ManiaDifficultyAttributes;

    fn new(beatmap: &'a Beatmap) -> Self {
        Self::new(beatmap)
    }
    fn mods(self, mods: u32) -> Self {
        self.mods(mods)
    }

    fn attributes(self, attrs: Self::Attrs) -> Self {
        self.attributes(attrs)
    }

    fn combo(self, _combo: usize) -> Self {
        // Mania doesn't seem to care about combo?
        self
    }

    fn accuracy(self, accuracy: f64) -> Self {
        self.accuracy(accuracy)
    }

    fn misses(self, misses: usize) -> Self {
        self.n_misses(misses)
    }

    fn get_pp(self) -> f64 {
        self.calculate().pp()
    }

    fn get_attrs(self) -> Self::Attrs {
        self.calculate().difficulty
    }
}

impl BeatmapContent {
    /// Get pp given the combo and accuracy.
    pub fn get_pp_from(
        &self,
        mode: Mode,
        combo: Option<usize>,
        accuracy: Accuracy,
        mods: Mods,
    ) -> Result<f64> {
        let bm = self.content.as_ref();
        Ok(match mode {
            Mode::Std => OsuPP::map_pp(bm, mods, combo, accuracy),
            Mode::Taiko => TaikoPP::map_pp(bm, mods, combo, accuracy),
            Mode::Catch => CatchPP::map_pp(bm, mods, combo, accuracy),
            Mode::Mania => ManiaPP::map_pp(bm, mods, combo, accuracy),
        })
    }

    /// Get info given mods.
    pub fn get_info_with(&self, mode: Mode, mods: Mods) -> Result<BeatmapInfo> {
        let bm = self.content.as_ref();
        Ok(match mode {
            Mode::Std => OsuPP::map_info(bm, mods),
            Mode::Taiko => TaikoPP::map_info(bm, mods),
            Mode::Catch => CatchPP::map_info(bm, mods),
            Mode::Mania => ManiaPP::map_info(bm, mods),
        })
    }

    pub fn get_possible_pp_with(&self, mode: Mode, mods: Mods) -> Result<BeatmapInfoWithPP> {
        let bm = self.content.as_ref();
        Ok(match mode {
            Mode::Std => OsuPP::map_info_with_pp(bm, mods),
            Mode::Taiko => TaikoPP::map_info_with_pp(bm, mods),
            Mode::Catch => CatchPP::map_info_with_pp(bm, mods),
            Mode::Mania => ManiaPP::map_info_with_pp(bm, mods),
        })
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
            content: Arc::new(Beatmap::parse(content.as_bytes())?),
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
