use chrono::{DateTime, Utc};
use mods::Stats;
use rosu_v2::prelude::GameModIntermode;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::fmt;
use std::time::Duration;

pub mod mods;
pub(crate) mod rosu;

pub use mods::Mods;
use serenity::utils::MessageBuilder;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum ApprovalStatus {
    Loved,
    Qualified,
    Approved,
    Ranked(DateTime<Utc>),
    Pending,
    WIP,
    Graveyarded,
}

impl fmt::Display for ApprovalStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let ApprovalStatus::Ranked(ref d) = self {
            write!(f, "Ranked on {}", d.format("<t:%s>"))
        } else {
            write!(f, "{:?}", self)
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Difficulty {
    pub stars: f64,
    pub aim: Option<f64>,
    pub speed: Option<f64>,

    pub cs: f64,
    pub od: f64,
    pub ar: f64,
    pub hp: f64,

    pub count_normal: u64,
    pub count_slider: u64,
    pub count_spinner: u64,
    pub max_combo: Option<u64>,

    pub bpm: f64,
    pub drain_length: Duration,
    pub total_length: Duration,
}

impl Difficulty {
    // Difficulty calculation is based on
    // https://www.reddit.com/r/osugame/comments/6phntt/difficulty_settings_table_with_all_values/
    //

    fn override_stats(&mut self, stats: &Stats) {
        self.cs = stats.cs.unwrap_or(self.cs);
        self.od = stats.od.unwrap_or(self.od);
        self.ar = stats.ar.unwrap_or(self.ar);
        self.hp = stats.hp.unwrap_or(self.hp);
    }

    fn apply_everything_by_ratio(&mut self, rat: f64) {
        self.cs = (self.cs * rat).min(10.0);
        self.od = (self.od * rat).min(10.0);
        self.ar = (self.ar * rat).min(10.0);
        self.hp = (self.hp * rat).min(10.0);
    }
    fn apply_ar_by_time_ratio(&mut self, rat: f64) {
        // Convert AR to approach time...
        let approach_time = if self.ar < 5.0 {
            1800.0 - self.ar * 120.0
        } else {
            1200.0 - (self.ar - 5.0) * 150.0
        };
        // Update it...
        let approach_time = approach_time * rat;
        // Convert it back to AR...
        self.ar = if approach_time > 1200.0 {
            (1800.0 - approach_time) / 120.0
        } else {
            (1200.0 - approach_time) / 150.0 + 5.0
        };
    }
    fn apply_od_by_time_ratio(&mut self, rat: f64) {
        // Convert OD to hit timing
        let hit_timing = 79.0 - self.od * 6.0 + 0.5;
        // Update it...
        let hit_timing = hit_timing * rat + 0.5 / rat;
        // then convert back
        self.od = (79.0 - (hit_timing - 0.5)) / 6.0;
    }
    fn apply_length_by_ratio(&mut self, ratio: f64) {
        self.bpm /= ratio; // Inverse since bpm increases while time decreases
        self.drain_length = Duration::from_secs_f64(self.drain_length.as_secs_f64() * ratio);
        self.total_length = Duration::from_secs_f64(self.total_length.as_secs_f64() * ratio);
    }
    /// Apply mods to the given difficulty.
    /// Note that `stars`, `aim` and `speed` cannot be calculated from this alone.
    pub fn apply_mods(&self, mods: &Mods, updated_stars: f64) -> Difficulty {
        let mut diff = Difficulty {
            stars: updated_stars,
            ..self.clone()
        };

        // Apply mods one by one
        if mods.inner.contains_intermode(GameModIntermode::Easy) {
            diff.apply_everything_by_ratio(0.5);
        }
        if mods.inner.contains_intermode(GameModIntermode::HardRock) {
            let old_cs = diff.cs;
            diff.apply_everything_by_ratio(1.4);
            // CS is changed by 1.3 tho
            diff.cs = old_cs * 1.3;
        }

        diff.override_stats(&mods.overrides());

        if let Some(ratio) = mods.inner.clock_rate() {
            if ratio != 1.0 {
                diff.apply_length_by_ratio(1.0 / ratio as f64);
                diff.apply_ar_by_time_ratio(1.0 / ratio as f64);
                diff.apply_od_by_time_ratio(1.0 / ratio as f64);
            }
        }

        diff
    }

    /// Format the difficulty info into a short summary.
    pub fn format_info<'a>(
        &self,
        mode: Mode,
        mods: &Mods,
        original_beatmap: impl Into<Option<&'a Beatmap>> + 'a,
    ) -> String {
        let original_beatmap = original_beatmap.into();
        let is_not_ranked = !matches!(
            original_beatmap.map(|v| v.approval),
            Some(ApprovalStatus::Ranked(_))
        );
        let three_lines = original_beatmap.is_some() && is_not_ranked;
        let bpm = (self.bpm * 100.0).round() / 100.0;
        MessageBuilder::new()
            .push(
                original_beatmap
                    .map(|original_beatmap| {
                        format!(
                            "[[Link]]({}) [[DL]]({}) [[B]({})|[C]({})] (`{}`)",
                            original_beatmap.link(),
                            original_beatmap.download_link(BeatmapSite::Bancho),
                            original_beatmap.download_link(BeatmapSite::Beatconnect),
                            original_beatmap.download_link(BeatmapSite::Chimu),
                            original_beatmap.short_link(Some(mode), mods)
                        )
                    })
                    .unwrap_or("**Uploaded**".to_owned()),
            )
            .push(if three_lines { "\n" } else { ", " })
            .push_bold(format!("{:.2}⭐", self.stars))
            .push(", ")
            .push(
                self.max_combo
                    .map(|c| format!("max **{}x**, ", c))
                    .unwrap_or_else(|| "".to_owned()),
            )
            .push(if is_not_ranked {
                format!(
                    "status **{}**, mode ",
                    original_beatmap
                        .map(|v| v.approval)
                        .unwrap_or(ApprovalStatus::WIP)
                )
            } else {
                "".to_owned()
            })
            .push_bold_line(format_mode(
                mode,
                original_beatmap.map(|v| v.mode).unwrap_or(mode),
            ))
            .push("CS")
            .push_bold(format!("{:.1}", self.cs))
            .push(", AR")
            .push_bold(format!("{:.1}", self.ar))
            .push(", OD")
            .push_bold(format!("{:.1}", self.od))
            .push(", HP")
            .push_bold(format!("{:.1}", self.hp))
            .push(format!(", BPM**{}**", bpm))
            .push(", ⌛ ")
            .push({
                let length = self.drain_length;
                let minutes = length.as_secs() / 60;
                let seconds = length.as_secs() % 60;
                format!("**{}:{:02}** (drain)", minutes, seconds)
            })
            .build()
    }
}

fn format_mode(actual: Mode, original: Mode) -> String {
    if actual == original {
        format!("{}", actual)
    } else {
        format!("{} (converted)", actual)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Deserialize, Serialize)]
pub enum Genre {
    Any,
    Unspecified,
    VideoGame,
    Anime,
    Rock,
    Pop,
    Other,
    Novelty,
    HipHop,
    Electronic,
    Metal,
    Classical,
    Folk,
    Jazz,
}

impl fmt::Display for Genre {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Genre::*;
        match self {
            VideoGame => write!(f, "Video Game"),
            HipHop => write!(f, "Hip Hop"),
            v => write!(f, "{:?}", v),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Language {
    Any,
    Other,
    English,
    Japanese,
    Chinese,
    Instrumental,
    Korean,
    French,
    German,
    Swedish,
    Spanish,
    Italian,
    Russian,
    Polish,
    Unspecified,
}

impl fmt::Display for Language {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, std::hash::Hash)]
pub enum Mode {
    Std,
    Taiko,
    Catch,
    Mania,
}

impl From<u8> for Mode {
    fn from(n: u8) -> Self {
        match n {
            0 => Self::Std,
            1 => Self::Taiko,
            2 => Self::Catch,
            3 => Self::Mania,
            _ => panic!("Unknown mode {}", n),
        }
    }
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Mode::*;
        write!(
            f,
            "{}",
            match self {
                Std => "osu!",
                Taiko => "osu!taiko",
                Mania => "osu!mania",
                Catch => "osu!catch",
            }
        )
    }
}

impl Mode {
    /// Parse from the display output of the enum itself.
    pub fn parse_from_display(s: &str) -> Option<Self> {
        Some(match s {
            "osu!" => Mode::Std,
            "osu!taiko" => Mode::Taiko,
            "osu!mania" => Mode::Mania,
            "osu!catch" => Mode::Catch,
            _ => return None,
        })
    }

    /// Parse from the new site's convention.
    pub fn parse_from_new_site(s: &str) -> Option<Self> {
        Some(match s {
            "osu" => Mode::Std,
            "taiko" => Mode::Taiko,
            "fruits" => Mode::Catch,
            "mania" => Mode::Mania,
            _ => return None,
        })
    }

    /// Returns the mode string in the new convention.
    pub fn as_str_new_site(&self) -> &'static str {
        match self {
            Mode::Std => "osu",
            Mode::Taiko => "taiko",
            Mode::Catch => "fruits",
            Mode::Mania => "mania",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Beatmap {
    // Beatmapset info
    pub approval: ApprovalStatus,
    pub submit_date: DateTime<Utc>,
    pub last_update: DateTime<Utc>,
    pub download_available: bool,
    pub audio_available: bool,
    // Media metadata
    pub artist: String,
    pub title: String,
    pub beatmapset_id: u64,
    pub creator: String,
    pub creator_id: u64,
    pub source: Option<String>,
    pub genre: Genre,
    pub language: Language,
    pub tags: Vec<String>,
    // Beatmap information
    pub beatmap_id: u64,
    pub difficulty_name: String,
    pub difficulty: Difficulty,
    pub file_hash: String,
    pub mode: Mode,
    pub favourite_count: u64,
    pub rating: f64,
    pub play_count: u64,
    pub pass_count: u64,
}

const NEW_MODE_NAMES: [&str; 4] = ["osu", "taiko", "fruits", "mania"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BeatmapSite {
    Bancho,
    Beatconnect,
    Chimu,
    OsuDirect,
}

impl BeatmapSite {
    pub fn download_link(self, beatmapset: u64) -> String {
        match self {
            BeatmapSite::Bancho => {
                format!("https://osu.ppy.sh/beatmapsets/{}/download", beatmapset)
            }
            BeatmapSite::Beatconnect => format!("https://beatconnect.io/b/{}", beatmapset),
            BeatmapSite::Chimu => format!("https://catboy.best/d/{}", beatmapset),
            BeatmapSite::OsuDirect => format!("osu://s/{}", beatmapset),
        }
    }
}

impl Beatmap {
    pub fn beatmapset_link(&self) -> String {
        format!(
            "https://osu.ppy.sh/beatmapsets/{}#{}",
            self.beatmapset_id, NEW_MODE_NAMES[self.mode as usize]
        )
    }

    /// Gets a link pointing to the beatmap, in the new format.
    pub fn link(&self) -> String {
        format!(
            "https://osu.ppy.sh/beatmapsets/{}#{}/{}",
            self.beatmapset_id, NEW_MODE_NAMES[self.mode as usize], self.beatmap_id
        )
    }

    /// Returns a direct download link. If `beatconnect` is true, return the beatconnect download link.
    pub fn download_link(&self, site: BeatmapSite) -> String {
        site.download_link(self.beatmapset_id)
    }

    /// Returns a direct link to the download (if you have supporter!)
    pub fn osu_direct_link(&self) -> String {
        format!("osu://b/{}", self.beatmapset_id)
    }

    /// Return a parsable short link.
    pub fn short_link(&self, override_mode: Option<Mode>, mods: &Mods) -> String {
        format!(
            "/b/{}{}{}",
            self.beatmap_id,
            match override_mode {
                Some(mode) if mode != self.mode => format!("/{}", mode.as_str_new_site()),
                _ => "".to_owned(),
            },
            mods.strip_lazer(override_mode.unwrap_or(Mode::Std))
        )
    }

    /// Link to the cover image of the beatmap.
    pub fn cover_url(&self) -> String {
        format!(
            "https://assets.ppy.sh/beatmaps/{}/covers/cover.jpg",
            self.beatmapset_id
        )
    }

    /// Link to the cover thumbnail of the beatmap.
    pub fn thumbnail_url(&self) -> String {
        format!("https://b.ppy.sh/thumb/{}l.jpg", self.beatmapset_id)
    }

    /// Beatmap title and difficulty name
    pub fn map_title(&self) -> String {
        MessageBuilder::new()
            .push_safe(&self.artist)
            .push(" - ")
            .push_safe(&self.title)
            .push(" [")
            .push_safe(&self.difficulty_name)
            .push("]")
            .build()
    }

    /// Full title with creator name if needed
    pub fn full_title(&self, mods: &Mods, stars: f64) -> String {
        let creator: Cow<str> = if self.difficulty_name.contains("'s") {
            "".into()
        } else {
            format!(" by {}", self.creator).into()
        };

        MessageBuilder::new()
            .push_safe(&self.artist)
            .push(" - ")
            .push_safe(&self.title)
            .push(" [")
            .push_safe(&self.difficulty_name)
            .push("] ")
            .push(mods.to_string())
            .push(format!(" ({:.2}\\*)", stars))
            .push_safe(creator)
            .build()
    }
}

#[derive(Clone, Debug)]
pub struct UserEvent(pub rosu_v2::model::event::Event);

/// Represents a "achieved rank #x on beatmap" event.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserEventRank {
    pub beatmap_id: u64,
    pub rank: u16,
    pub mode: Mode,
    pub date: DateTime<Utc>,
}

impl UserEvent {
    /// Try to parse the event into a "rank" event.
    pub fn to_event_rank(&self) -> Option<UserEventRank> {
        match &self.0.event_type {
            rosu_v2::model::event::EventType::Rank {
                grade: _,
                rank,
                mode,
                beatmap,
                user: _,
            } => Some(UserEventRank {
                beatmap_id: {
                    beatmap
                        .url
                        .trim_start_matches("/b/")
                        .trim_end_matches("?m=0")
                        .trim_end_matches("?m=1")
                        .trim_end_matches("?m=2")
                        .trim_end_matches("?m=3")
                        .parse::<u64>()
                        .unwrap()
                },
                rank: *rank as u16,
                mode: (*mode).into(),
                date: rosu::time_to_utc(self.0.created_at),
            }),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct UserHeader {
    pub id: u64,
    pub username: String,
}

#[derive(Clone, Debug)]
pub struct User {
    pub id: u64,
    pub username: String,
    pub joined: DateTime<Utc>,
    pub country: String,
    pub preferred_mode: Mode,
    // History
    pub count_300: u64,
    pub count_100: u64,
    pub count_50: u64,
    pub play_count: u64,
    pub played_time: Duration,
    pub ranked_score: u64,
    pub total_score: u64,
    pub count_ss: u64,
    pub count_ssh: u64,
    pub count_s: u64,
    pub count_sh: u64,
    pub count_a: u64,
    pub events: Vec<UserEvent>,
    // Rankings
    pub rank: u64,
    pub country_rank: u64,
    pub level: f64,
    pub pp: Option<f64>,
    pub accuracy: f64,
}

impl User {
    pub fn link(&self) -> String {
        format!("https://osu.ppy.sh/users/{}", self.id)
    }

    pub fn avatar_url(&self) -> String {
        format!("https://a.ppy.sh/{}", self.id)
    }
}

impl UserHeader {
    pub fn link(&self) -> String {
        format!("https://osu.ppy.sh/users/{}", self.id)
    }

    pub fn avatar_url(&self) -> String {
        format!("https://a.ppy.sh/{}", self.id)
    }
}

impl<'a> From<&'a User> for UserHeader {
    fn from(u: &'a User) -> Self {
        Self {
            id: u.id,
            username: u.username.clone(),
        }
    }
}

impl From<User> for UserHeader {
    fn from(u: User) -> Self {
        Self {
            id: u.id,
            username: u.username,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum Rank {
    SS,
    SSH,
    S,
    SH,
    A,
    B,
    C,
    D,
    F,
}

impl std::str::FromStr for Rank {
    type Err = String;
    fn from_str(a: &str) -> Result<Self, Self::Err> {
        Ok(match &a.to_uppercase()[..] {
            "SS" | "X" => Rank::SS,
            "SSH" | "XH" => Rank::SSH,
            "S" => Rank::S,
            "SH" => Rank::SH,
            "A" => Rank::A,
            "B" => Rank::B,
            "C" => Rank::C,
            "D" => Rank::D,
            "F" => Rank::F,
            t => return Err(format!("Invalid value {}", t)),
        })
    }
}

impl fmt::Display for Rank {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Clone, Debug)]
pub struct Score {
    pub id: Option<u64>, // No id if you fail
    pub user_id: u64,
    pub date: DateTime<Utc>,
    pub replay_available: bool,
    pub beatmap_id: u64,

    pub score: Option<u64>,
    pub normalized_score: u32,
    pub pp: Option<f64>,
    pub rank: Rank,
    pub mode: Mode,
    pub mods: Mods, // Later

    pub count_300: u64,
    pub count_100: u64,
    pub count_50: u64,
    pub count_miss: u64,
    pub count_katu: u64,
    pub count_geki: u64,
    pub max_combo: u64,
    pub perfect: bool,

    /// Whether score would get pp
    pub ranked: Option<bool>,
    /// Whether score would be stored
    pub preserved: Option<bool>,

    // Some APIv2 stats
    pub server_accuracy: f64,
    pub global_rank: Option<u32>,
    pub effective_pp: Option<f64>,

    pub lazer_build_id: Option<u32>,
}

impl Score {
    /// Given the play's mode, calculate the score's accuracy.
    pub fn accuracy(&self, _mode: Mode) -> f64 {
        self.server_accuracy
    }

    /// Gets the link to the score, if it exists.
    pub fn link(&self) -> Option<String> {
        self.id
            .map(|id| format!("https://osu.ppy.sh/scores/{}", id))
    }

    /// Gets the link to the replay, if it exists.
    pub fn replay_download_link(&self) -> Option<String> {
        let id = self.id?;
        if self.replay_available {
            Some(format!("https://osu.ppy.sh/scores/{}/download", id))
        } else {
            None
        }
    }
}
