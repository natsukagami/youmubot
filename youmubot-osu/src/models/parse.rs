use super::*;
use chrono::{
    format::{parse, Item, Numeric, Pad, Parsed},
    DateTime, ParseError as ChronoParseError, Utc,
};
use std::convert::TryFrom;
use std::time::Duration;
use std::{error::Error, fmt, str::FromStr};

/// Errors that can be identified from parsing.
#[derive(Debug)]
pub enum ParseError {
    InvalidValue { field: &'static str, value: String },
    FromStr(String),
    NoApprovalDate,
    DateParseError(ChronoParseError),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use ParseError::*;
        match self {
            InvalidValue {
                ref field,
                ref value,
            } => write!(f, "Invalid value `{}` for {}", value, field),
            FromStr(ref s) => write!(f, "Invalid value `{}` parsing from string", s),
            NoApprovalDate => write!(f, "Approval date expected but not found"),
            DateParseError(ref r) => write!(f, "Error parsing date: {}", r),
        }
    }
}

impl Error for ParseError {}

type ParseResult<T> = Result<T, ParseError>;

impl TryFrom<raw::Score> for Score {
    type Error = ParseError;
    fn try_from(raw: raw::Score) -> Result<Self, Self::Error> {
        Ok(Score {
            id: raw.score_id.map(parse_from_str).transpose()?,
            user_id: parse_from_str(&raw.user_id)?,
            date: parse_date(&raw.date)?,
            beatmap_id: raw.beatmap_id.map(parse_from_str).transpose()?.unwrap_or(0),
            replay_available: raw
                .replay_available
                .map(parse_bool)
                .transpose()?
                .unwrap_or(false),
            score: parse_from_str(&raw.score)?,
            pp: raw.pp.map(parse_from_str).transpose()?,
            rank: parse_from_str(&raw.rank)?,
            mods: {
                let v: u64 = parse_from_str(&raw.enabled_mods)?;
                Mods::from_bits(v).unwrap_or(Mods::NOMOD)
            },
            count_300: parse_from_str(&raw.count300)?,
            count_100: parse_from_str(&raw.count100)?,
            count_50: parse_from_str(&raw.count50)?,
            count_miss: parse_from_str(&raw.countmiss)?,
            count_katu: parse_from_str(&raw.countkatu)?,
            count_geki: parse_from_str(&raw.countgeki)?,
            max_combo: parse_from_str(&raw.maxcombo)?,
            perfect: parse_bool(&raw.perfect)?,
        })
    }
}

impl TryFrom<raw::User> for User {
    type Error = ParseError;
    fn try_from(raw: raw::User) -> Result<Self, Self::Error> {
        Ok(User {
            id: parse_from_str(&raw.user_id)?,
            username: raw.username,
            joined: parse_date(&raw.join_date)?,
            country: raw.country,
            count_300: raw.count300.map(parse_from_str).unwrap_or(Ok(0))?,
            count_100: raw.count100.map(parse_from_str).unwrap_or(Ok(0))?,
            count_50: raw.count50.map(parse_from_str).unwrap_or(Ok(0))?,
            play_count: raw.playcount.map(parse_from_str).unwrap_or(Ok(0))?,
            played_time: raw
                .total_seconds_played
                .map(parse_duration)
                .unwrap_or(Ok(Duration::from_secs(0)))?,
            ranked_score: raw.ranked_score.map(parse_from_str).unwrap_or(Ok(0))?,
            total_score: raw.total_score.map(parse_from_str).unwrap_or(Ok(0))?,
            count_ss: raw.count_rank_ss.map(parse_from_str).unwrap_or(Ok(0))?,
            count_ssh: raw.count_rank_ssh.map(parse_from_str).unwrap_or(Ok(0))?,
            count_s: raw.count_rank_s.map(parse_from_str).unwrap_or(Ok(0))?,
            count_sh: raw.count_rank_sh.map(parse_from_str).unwrap_or(Ok(0))?,
            count_a: raw.count_rank_a.map(parse_from_str).unwrap_or(Ok(0))?,
            rank: raw.pp_rank.map(parse_from_str).unwrap_or(Ok(0))?,
            country_rank: raw.pp_country_rank.map(parse_from_str).unwrap_or(Ok(0))?,
            level: raw.level.map(parse_from_str).unwrap_or(Ok(0.0))?,
            pp: Some(raw.pp_raw.map(parse_from_str).unwrap_or(Ok(0.0))?).filter(|v| *v != 0.0),
            accuracy: raw.accuracy.map(parse_from_str).unwrap_or(Ok(0.0))?,
            events: {
                let mut v = Vec::new();
                for e in raw.events.into_iter() {
                    v.push(parse_user_event(e)?);
                }
                v
            },
        })
    }
}

impl TryFrom<raw::Beatmap> for Beatmap {
    type Error = ParseError;
    fn try_from(raw: raw::Beatmap) -> Result<Self, Self::Error> {
        Ok(Beatmap {
            approval: parse_approval_status(&raw)?,
            submit_date: parse_date(&raw.submit_date)?,
            last_update: parse_date(&raw.last_update)?,
            download_available: !(parse_bool(&raw.download_unavailable)?),
            audio_available: !(parse_bool(&raw.audio_unavailable)?),
            artist: raw.artist,
            beatmap_id: parse_from_str(&raw.beatmap_id)?,
            beatmapset_id: parse_from_str(&raw.beatmapset_id)?,
            title: raw.title,
            bpm: parse_from_str(&raw.bpm)?,
            creator: raw.creator,
            creator_id: parse_from_str(&raw.creator_id)?,
            source: raw.source.filter(|v| !v.is_empty()),
            genre: parse_genre(&raw.genre_id)?,
            language: parse_language(&raw.language_id)?,
            tags: raw.tags.split_whitespace().map(|v| v.to_owned()).collect(),
            difficulty_name: raw.version,
            difficulty: Difficulty {
                stars: parse_from_str(&raw.difficultyrating)?,
                aim: raw.diff_aim.map(parse_from_str).transpose()?,
                speed: raw.diff_speed.map(parse_from_str).transpose()?,
                cs: parse_from_str(&raw.diff_size)?,
                od: parse_from_str(&raw.diff_overall)?,
                ar: parse_from_str(&raw.diff_approach)?,
                hp: parse_from_str(&raw.diff_drain)?,
                count_normal: parse_from_str(&raw.count_normal)?,
                count_slider: parse_from_str(&raw.count_slider)?,
                count_spinner: parse_from_str(&raw.count_spinner)?,
                max_combo: raw.max_combo.map(parse_from_str).transpose()?,
                drain_length: parse_duration(&raw.hit_length)?,
                total_length: parse_duration(&raw.total_length)?,
            },
            file_hash: raw.file_md5,
            mode: parse_mode(&raw.mode)?,
            favourite_count: parse_from_str(&raw.favourite_count)?,
            rating: parse_from_str(&raw.rating)?,
            play_count: parse_from_str(&raw.playcount)?,
            pass_count: parse_from_str(&raw.passcount)?,
        })
    }
}

fn parse_user_event(s: raw::UserEvent) -> ParseResult<UserEvent> {
    Ok(UserEvent {
        display_html: s.display_html,
        beatmap_id: s.beatmap_id.map(parse_from_str).transpose()?,
        beatmapset_id: s.beatmapset_id.map(parse_from_str).transpose()?,
        date: parse_date(&s.date)?,
        epic_factor: parse_from_str(&s.epicfactor)?,
    })
}

fn parse_mode(s: impl AsRef<str>) -> ParseResult<Mode> {
    let t: u8 = parse_from_str(s)?;
    use Mode::*;
    Ok(match t {
        0 => Std,
        1 => Taiko,
        2 => Catch,
        3 => Mania,
        _ => {
            return Err(ParseError::InvalidValue {
                field: "mode",
                value: t.to_string(),
            })
        }
    })
}

fn parse_language(s: impl AsRef<str>) -> ParseResult<Language> {
    let t: u8 = parse_from_str(s)?;
    use Language::*;
    Ok(match t {
        0 => Any,
        1 | 14 => Other,
        2 => English,
        3 => Japanese,
        4 => Chinese,
        5 => Instrumental,
        6 => Korean,
        7 => French,
        8 => German,
        9 => Swedish,
        10 => Spanish,
        11 => Italian,
        _ => {
            return Err(ParseError::InvalidValue {
                field: "language",
                value: t.to_string(),
            })
        }
    })
}

fn parse_genre(s: impl AsRef<str>) -> ParseResult<Genre> {
    let t: u8 = parse_from_str(s)?;
    use Genre::*;
    Ok(match t {
        0 => Any,
        1 => Unspecified,
        2 => VideoGame,
        3 => Anime,
        4 => Rock,
        5 => Pop,
        6 => Other,
        7 => Novelty,
        9 => HipHop,
        10 => Electronic,
        13 => Folk,
        _ => {
            return Err(ParseError::InvalidValue {
                field: "genre",
                value: t.to_string(),
            })
        }
    })
}

fn parse_duration(s: impl AsRef<str>) -> ParseResult<Duration> {
    Ok(Duration::from_secs(parse_from_str(s)?))
}

fn parse_from_str<T: FromStr>(s: impl AsRef<str>) -> ParseResult<T> {
    let v = s.as_ref();
    T::from_str(v).map_err(|_| ParseError::FromStr(v.to_owned()))
}

fn parse_bool(b: impl AsRef<str>) -> ParseResult<bool> {
    match b.as_ref() {
        "1" => Ok(true),
        "0" => Ok(false),
        t => Err(ParseError::InvalidValue {
            field: "bool",
            value: t.to_owned(),
        }),
    }
}

fn parse_approval_status(b: &raw::Beatmap) -> ParseResult<ApprovalStatus> {
    use ApprovalStatus::*;
    Ok(match &b.approved[..] {
        "4" => Loved,
        "3" => Qualified,
        "2" => Approved,
        "1" => Ranked(parse_date(
            b.approved_date.as_ref().ok_or(ParseError::NoApprovalDate)?,
        )?),
        "0" => Pending,
        "-1" => WIP,
        "-2" => Graveyarded,
        t => {
            return Err(ParseError::InvalidValue {
                field: "approval status",
                value: t.to_owned(),
            })
        }
    })
}

fn parse_date(date: impl AsRef<str>) -> ParseResult<DateTime<Utc>> {
    let mut parsed = Parsed::new();
    parse(
        &mut parsed,
        date.as_ref(),
        (&[
            Item::Numeric(Numeric::Year, Pad::Zero),
            Item::Literal("-"),
            Item::Numeric(Numeric::Month, Pad::Zero),
            Item::Literal("-"),
            Item::Numeric(Numeric::Day, Pad::Zero),
            Item::Space(""),
            Item::Numeric(Numeric::Hour, Pad::Zero),
            Item::Literal(":"),
            Item::Numeric(Numeric::Minute, Pad::Zero),
            Item::Literal(":"),
            Item::Numeric(Numeric::Second, Pad::Zero),
        ])
            .iter(),
    )
    .map_err(ParseError::DateParseError)?;
    parsed
        .to_datetime_with_timezone(&Utc {})
        .map_err(ParseError::DateParseError)
}
