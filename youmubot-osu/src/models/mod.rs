use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::Duration;

pub mod mods;
pub mod parse;
pub(crate) mod raw;

pub use mods::Mods;

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
            write!(f, "Ranked on {}", d.format("%F %T"))
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
}

impl fmt::Display for Language {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    Std,
    Taiko,
    Catch,
    Mania,
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
    pub bpm: f64,
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
    pub drain_length: Duration,
    pub total_length: Duration,
    pub file_hash: String,
    pub mode: Mode,
    pub favourite_count: u64,
    pub rating: f64,
    pub play_count: u64,
    pub pass_count: u64,
}

const NEW_MODE_NAMES: [&'static str; 4] = ["osu", "taiko", "fruits", "mania"];

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

    /// Link to the cover image of the beatmap.
    pub fn cover_url(&self) -> String {
        format!(
            "https://assets.ppy.sh/beatmaps/{}/covers/cover.jpg",
            self.beatmapset_id
        )
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserEvent {
    pub display_html: String,
    pub beatmap_id: u64,
    pub beatmapset_id: u64,
    pub date: DateTime<Utc>,
    pub epic_factor: u8,
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
