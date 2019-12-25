use super::*;
use chrono::{
    format::{parse, Item, Numeric, Pad, Parsed},
    DateTime, Duration, Utc,
};
use serde::{de, Deserialize, Deserializer};
use std::str::FromStr;

impl<'de> Deserialize<'de> for User {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw: raw::User = raw::User::deserialize(deserializer)?;
        Ok(User {
            id: parse_from_str(&raw.user_id)?,
            username: raw.username,
            joined: parse_date(&raw.join_date)?,
            country: raw.country,
            count_300: parse_from_str(&raw.count300)?,
            count_100: parse_from_str(&raw.count100)?,
            count_50: parse_from_str(&raw.count50)?,
            play_count: parse_from_str(&raw.playcount)?,
            played_time: parse_duration(&raw.total_seconds_played)?,
            ranked_score: parse_from_str(&raw.ranked_score)?,
            total_score: parse_from_str(&raw.total_score)?,
            count_ss: parse_from_str(&raw.count_rank_ss)?,
            count_ssh: parse_from_str(&raw.count_rank_ssh)?,
            count_s: parse_from_str(&raw.count_rank_s)?,
            count_sh: parse_from_str(&raw.count_rank_sh)?,
            count_a: parse_from_str(&raw.count_rank_a)?,
            rank: parse_from_str(&raw.pp_rank)?,
            country_rank: parse_from_str(&raw.pp_country_rank)?,
            level: parse_from_str(&raw.level)?,
            pp: Some(parse_from_str(&raw.pp_raw)?).filter(|v| *v != 0.0),
            accuracy: parse_from_str(&raw.accuracy)?,
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

impl<'de> Deserialize<'de> for Beatmap {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw: raw::Beatmap = raw::Beatmap::deserialize(deserializer)?;
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
            },
            drain_length: parse_duration(&raw.hit_length)?,
            total_length: parse_duration(&raw.total_length)?,
            file_hash: raw.file_md5,
            mode: parse_mode(&raw.mode)?,
            favourite_count: parse_from_str(&raw.favourite_count)?,
            rating: parse_from_str(&raw.rating)?,
            play_count: parse_from_str(&raw.playcount)?,
            pass_count: parse_from_str(&raw.passcount)?,
        })
    }
}

fn parse_user_event<E: de::Error>(s: raw::UserEvent) -> Result<UserEvent, E> {
    Ok(UserEvent {
        display_html: s.display_html,
        beatmap_id: parse_from_str(&s.beatmap_id)?,
        beatmapset_id: parse_from_str(&s.beatmapset_id)?,
        date: parse_date(&s.date)?,
        epic_factor: parse_from_str(&s.epicfactor)?,
    })
}

fn parse_mode<E: de::Error>(s: impl AsRef<str>) -> Result<Mode, E> {
    let t: u8 = parse_from_str(s)?;
    use Mode::*;
    Ok(match t {
        0 => Std,
        1 => Taiko,
        2 => Catch,
        3 => Mania,
        _ => return Err(E::custom(format!("invalid value {} for mode", t))),
    })
}

fn parse_language<E: de::Error>(s: impl AsRef<str>) -> Result<Language, E> {
    let t: u8 = parse_from_str(s)?;
    use Language::*;
    Ok(match t {
        0 => Any,
        1 => Other,
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
        _ => return Err(E::custom(format!("invalid value {} for language", t))),
    })
}

fn parse_genre<E: de::Error>(s: impl AsRef<str>) -> Result<Genre, E> {
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
        _ => return Err(E::custom(format!("invalid value {} for genre", t))),
    })
}

fn parse_duration<E: de::Error>(s: impl AsRef<str>) -> Result<Duration, E> {
    Ok(Duration::seconds(parse_from_str(s)?))
}

fn parse_from_str<T: FromStr, E: de::Error>(s: impl AsRef<str>) -> Result<T, E> {
    T::from_str(s.as_ref()).map_err(|_| E::custom(format!("invalid value {}", s.as_ref())))
}

fn parse_bool<E: de::Error>(b: impl AsRef<str>) -> Result<bool, E> {
    match b.as_ref() {
        "1" => Ok(true),
        "0" => Ok(false),
        _ => Err(E::custom("invalid value for bool")),
    }
}

fn parse_approval_status<E: de::Error>(b: &raw::Beatmap) -> Result<ApprovalStatus, E> {
    use ApprovalStatus::*;
    Ok(match &b.approved[..] {
        "4" => Loved,
        "3" => Qualified,
        "2" => Approved,
        "1" => Ranked(parse_date(
            b.approved_date
                .as_ref()
                .ok_or(E::custom("expected approved date got none"))?,
        )?),
        "0" => Pending,
        "-1" => WIP,
        "-2" => Graveyarded,
        _ => return Err(E::custom("invalid value for approval status")),
    })
}

fn parse_date<E: de::Error>(date: impl AsRef<str>) -> Result<DateTime<Utc>, E> {
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
    .map_err(E::custom)?;
    parsed.to_datetime_with_timezone(&Utc {}).map_err(E::custom)
}
