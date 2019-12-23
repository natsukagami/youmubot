use chrono::{DateTime, Duration, Utc};
use std::string::ToString;

pub mod deser;
pub(crate) mod raw;

#[derive(Debug)]
pub enum ApprovalStatus {
    Loved,
    Qualified,
    Approved,
    Ranked(DateTime<Utc>),
    Pending,
    WIP,
    Graveyarded,
}

#[derive(Debug)]
pub struct Difficulty {
    pub stars: f64,
    pub aim: f64,
    pub speed: f64,

    pub cs: f64,
    pub od: f64,
    pub ar: f64,
    pub hp: f64,

    pub count_normal: u64,
    pub count_slider: u64,
    pub count_spinner: u64,
    pub max_combo: u64,
}

#[derive(Debug)]
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

#[derive(Debug)]
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
#[derive(Clone, Copy, Debug)]
pub enum Mode {
    Std,
    Taiko,
    Mania,
    Catch,
}

impl ToString for Mode {
    fn to_string(&self) -> String {
        (*self as u64).to_string()
    }
}

#[derive(Debug)]
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

pub struct UserEvent {
    pub display_html: String,
    pub beatmap_id: u64,
    pub beatmapset_id: u64,
    pub date: DateTime<Utc>,
    pub epic_factor: u8,
}

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
    pub level: f64,
    pub pp: Option<u64>,
    pub accuracy: f64,
}

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

pub struct Score {
    pub id: u64,
    pub username: String,
    pub user_id: u64,
    pub date: DateTime<Utc>,
    pub replay_available: bool,

    pub score: u64,
    pub pp: f64,
    pub rank: Rank,
    pub mods: u64, // Later

    pub count_300: u64,
    pub count_100: u64,
    pub count_50: u64,
    pub count_miss: u64,
    pub count_katu: u64,
    pub count_geki: u64,
    pub max_combo: u64,
    pub perfect: bool,
}
