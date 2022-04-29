use chrono::{DateTime, Utc};
use regex::Regex;
use rosu_pp::GameMode;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::Duration;

pub mod mods;
pub mod parse;
pub(crate) mod raw;

pub use mods::Mods;
use serenity::utils::MessageBuilder;

lazy_static::lazy_static! {
    static ref EVENT_RANK_REGEX: Regex = Regex::new(r#"^.+achieved .*rank #(\d+).* on .+\((.+)\)$"#).unwrap();
}

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
    fn apply_length_by_ratio(&mut self, mul: u32, div: u32) {
        self.bpm = self.bpm / (mul as f64) * (div as f64); // Inverse since bpm increases while time decreases
        self.drain_length = self.drain_length * mul / div;
        self.total_length = self.total_length * mul / div;
    }
    /// Apply mods to the given difficulty.
    /// Note that `stars`, `aim` and `speed` cannot be calculated from this alone.
    pub fn apply_mods(&self, mods: Mods, updated_stars: Option<f64>) -> Difficulty {
        let mut diff = Difficulty {
            stars: updated_stars.unwrap_or(self.stars),
            ..self.clone()
        };

        // Apply mods one by one
        if mods.contains(Mods::EZ) {
            diff.apply_everything_by_ratio(0.5);
        }
        if mods.contains(Mods::HR) {
            let old_cs = diff.cs;
            diff.apply_everything_by_ratio(1.4);
            // CS is changed by 1.3 tho
            diff.cs = old_cs * 1.3;
        }
        if mods.contains(Mods::HT) {
            diff.apply_ar_by_time_ratio(4.0 / 3.0);
            diff.apply_od_by_time_ratio(4.0 / 3.0);
            diff.apply_length_by_ratio(4, 3);
        }
        if mods.contains(Mods::DT) {
            diff.apply_ar_by_time_ratio(2.0 / 3.0);
            diff.apply_od_by_time_ratio(2.0 / 3.0);
            diff.apply_length_by_ratio(2, 3);
        }

        diff
    }

    /// Format the difficulty info into a short summary.
    pub fn format_info<'a>(
        &self,
        mode: Mode,
        mods: Mods,
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
                            "[[Link]]({}) [[DL]]({}) [[Alt]]({}) (`{}`)",
                            original_beatmap.link(),
                            original_beatmap.download_link(false),
                            original_beatmap.download_link(true),
                            original_beatmap.short_link(Some(mode), Some(mods))
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

impl From<Mode> for GameMode {
    fn from(n: Mode) -> Self {
        match n {
            Mode::Std => GameMode::STD,
            Mode::Taiko => GameMode::TKO,
            Mode::Catch => GameMode::CTB,
            Mode::Mania => GameMode::MNA,
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
            "osu!mania" => Mode::Catch,
            "osu!catch" => Mode::Mania,
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

    /// Returns a direct download link. If `bloodcat` is true, return the bloodcat download link.
    pub fn download_link(&self, bloodcat: bool) -> String {
        if bloodcat {
            format!("https://bloodcat.com/osu/s/{}", self.beatmapset_id)
        } else {
            format!(
                "https://osu.ppy.sh/beatmapsets/{}/download",
                self.beatmapset_id
            )
        }
    }

    /// Returns a direct link to the download (if you have supporter!)
    pub fn osu_direct_link(&self) -> String {
        format!("osu://b/{}", self.beatmapset_id)
    }

    /// Return a parsable short link.
    pub fn short_link(&self, override_mode: Option<Mode>, mods: Option<Mods>) -> String {
        format!(
            "/b/{}{}{}",
            self.beatmap_id,
            match override_mode {
                Some(mode) if mode != self.mode => format!("/{}", mode.as_str_new_site()),
                _ => "".to_owned(),
            },
            mods.map(|m| format!("{}", m))
                .unwrap_or_else(|| "".to_owned()),
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
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserEvent {
    pub display_html: String,
    pub beatmap_id: Option<u64>,
    pub beatmapset_id: Option<u64>,
    pub date: DateTime<Utc>,
    pub epic_factor: u8,
}

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
        let captures = EVENT_RANK_REGEX.captures(self.display_html.as_str())?;
        let rank: u16 = captures.get(1)?.as_str().parse().ok()?;
        let mode: Mode = Mode::parse_from_display(captures.get(2)?.as_str())?;
        Some(UserEventRank {
            beatmap_id: self.beatmap_id?,
            date: self.date,
            mode,
            rank,
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct User {
    pub id: u64,
    pub username: String,
    pub joined: DateTime<Utc>,
    pub country: String,
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Score {
    pub id: Option<u64>, // No id if you fail
    pub user_id: u64,
    pub date: DateTime<Utc>,
    pub replay_available: bool,
    pub beatmap_id: u64,

    pub score: u64,
    pub pp: Option<f64>,
    pub rank: Rank,
    pub mods: Mods, // Later

    pub count_300: u64,
    pub count_100: u64,
    pub count_50: u64,
    pub count_miss: u64,
    pub count_katu: u64,
    pub count_geki: u64,
    pub max_combo: u64,
    pub perfect: bool,
}

impl Score {
    /// Given the play's mode, calculate the score's accuracy.
    pub fn accuracy(&self, mode: Mode) -> f64 {
        100.0
            * match mode {
                Mode::Std => {
                    (6 * self.count_300 + 2 * self.count_100 + self.count_50) as f64
                        / (6.0
                            * (self.count_300 + self.count_100 + self.count_50 + self.count_miss)
                                as f64)
                }
                Mode::Taiko => {
                    (2 * self.count_300 + self.count_100) as f64
                        / 2.0
                        / (self.count_300 + self.count_100 + self.count_miss) as f64
                }
                Mode::Catch => {
                    (self.count_300 + self.count_100) as f64
                        / (self.count_300 + self.count_100 + self.count_miss + self.count_katu/* # of droplet misses */)
                            as f64
                }
                Mode::Mania => {
                    ((self.count_geki /* MAX */ + self.count_300) * 6
                        + self.count_katu /* 200 */ * 4
                        + self.count_100 * 2
                        + self.count_50) as f64
                        / 6.0
                        / (self.count_geki
                            + self.count_300
                            + self.count_katu
                            + self.count_100
                            + self.count_50
                            + self.count_miss) as f64
                }
            }
    }
}
