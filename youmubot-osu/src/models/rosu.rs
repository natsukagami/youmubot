use rosu_v2::model::{self as rosu};

use super::*;

impl ApprovalStatus {
    pub(crate) fn from_rosu(
        rank_status: rosu_v2::model::beatmap::RankStatus,
        ranked_date: Option<DateTime<Utc>>,
    ) -> Self {
        use ApprovalStatus::*;
        match rank_status {
            rosu_v2::model::beatmap::RankStatus::Graveyard => Graveyarded,
            rosu_v2::model::beatmap::RankStatus::WIP => WIP,
            rosu_v2::model::beatmap::RankStatus::Pending => Pending,
            rosu_v2::model::beatmap::RankStatus::Ranked => Ranked(ranked_date.unwrap()),
            rosu_v2::model::beatmap::RankStatus::Approved => Approved,
            rosu_v2::model::beatmap::RankStatus::Qualified => Qualified,
            rosu_v2::model::beatmap::RankStatus::Loved => Loved,
        }
    }
}

pub(super) fn time_to_utc(s: time::OffsetDateTime) -> DateTime<Utc> {
    chrono::DateTime::from_timestamp(s.unix_timestamp(), 0).unwrap()
}

impl Beatmap {
    pub(crate) fn from_rosu(
        bm: rosu::beatmap::BeatmapExtended,
        set: &rosu::beatmap::BeatmapsetExtended,
    ) -> Self {
        let last_updated = time_to_utc(bm.last_updated);
        let difficulty = Difficulty::from_rosu(&bm);
        Self {
            approval: ApprovalStatus::from_rosu(bm.status, set.ranked_date.map(time_to_utc)),
            submit_date: set.submitted_date.map(time_to_utc).unwrap_or(last_updated),
            last_update: last_updated,
            download_available: !set.availability.download_disabled, // don't think we have this stat
            audio_available: !set.availability.download_disabled,    // neither is this
            artist: set.artist.clone(),
            title: set.title.clone(),
            beatmapset_id: set.mapset_id as u64,
            creator: set.creator_name.clone().into_string(),
            creator_id: set.creator_id as u64,
            source: Some(set.source.clone()).filter(|s| !s.is_empty()).clone(),
            genre: set.genre.map(|v| v.into()).unwrap_or(Genre::Unspecified),
            language: set.language.map(|v| v.into()).unwrap_or(Language::Any),
            tags: set.tags.split(", ").map(|v| v.to_owned()).collect(),
            beatmap_id: bm.map_id as u64,
            difficulty_name: bm.version,
            difficulty,
            file_hash: bm.checksum.unwrap_or_else(|| "none".to_owned()),
            mode: bm.mode.into(),
            favourite_count: set.favourite_count as u64,
            rating: set
                .ratings
                .as_ref()
                .map(|rs| {
                    (rs.iter()
                        .enumerate()
                        .map(|(r, id)| ((r + 1) as u32 * *id))
                        .sum::<u32>()) as f64
                        / (rs.iter().sum::<u32>() as f64)
                })
                .unwrap_or(0.0),
            play_count: bm.playcount as u64,
            pass_count: bm.passcount as u64,
        }
    }
}

impl User {
    pub(crate) fn from_rosu(
        user: rosu::user::UserExtended,
        stats: rosu::user::UserStatistics,
    ) -> Self {
        Self {
            id: user.user_id as u64,
            username: user.username.into_string(),
            joined: time_to_utc(user.join_date),
            country: user.country_code.to_string(),
            preferred_mode: user.mode.into(),
            count_300: 0, // why do we even want this
            count_100: 0, // why do we even want this
            count_50: 0,  // why do we even want this
            play_count: stats.playcount as u64,
            played_time: Duration::from_secs(stats.playtime as u64),
            ranked_score: stats.ranked_score,
            total_score: stats.total_score,
            count_ss: stats.grade_counts.ss as u64,
            count_ssh: stats.grade_counts.ssh as u64,
            count_s: stats.grade_counts.s as u64,
            count_sh: stats.grade_counts.sh as u64,
            count_a: stats.grade_counts.a as u64,
            rank: stats.global_rank.unwrap_or(0) as u64,
            country_rank: stats.country_rank.unwrap_or(0) as u64,
            level: stats.level.current as f64 + stats.level.progress as f64 / 100.0,
            pp: Some(stats.pp as f64),
            accuracy: stats.accuracy as f64,
        }
    }
}

impl From<rosu::event::Event> for UserEvent {
    fn from(value: rosu::event::Event) -> Self {
        Self(value)
    }
}

impl From<rosu::score::Score> for Score {
    fn from(s: rosu::score::Score) -> Self {
        let legacy_stats = s.statistics.as_legacy(s.mode);
        let score = if s.set_on_lazer {
            s.score as u64
        } else {
            s.classic_score
        };
        Self {
            id: Some(s.id),
            user_id: s.user_id as u64,
            date: time_to_utc(s.ended_at),
            replay_available: s.replay,
            beatmap_id: s.map_id as u64,
            score,
            normalized_score: s.score,
            pp: s.pp.map(|v| v as f64),
            rank: if s.passed { s.grade.into() } else { Rank::F },
            server_accuracy: s.accuracy as f64,
            global_rank: s.rank_global,
            effective_pp: s.weight.map(|w| w.pp as f64),
            mode: s.mode.into(),
            mods: Mods::from_gamemods(s.mods, s.set_on_lazer),
            count_300: legacy_stats.count_300 as u64,
            count_100: legacy_stats.count_100 as u64,
            count_50: legacy_stats.count_50 as u64,
            count_miss: legacy_stats.count_miss as u64,
            count_katu: legacy_stats.count_katu as u64,
            count_geki: legacy_stats.count_geki as u64,
            statistics: s.statistics,
            max_combo: s.max_combo,
            perfect: s.is_perfect_combo,
            ranked: s.ranked,
            preserved: s.preserve,
            lazer_build_id: s.build_id,
        }
    }
}

impl Difficulty {
    pub(crate) fn from_rosu(bm: &rosu::beatmap::BeatmapExtended) -> Self {
        Self {
            stars: bm.stars as f64,
            aim: None,
            speed: None,
            cs: bm.cs as f64,
            od: bm.od as f64,
            ar: bm.ar as f64,
            hp: bm.hp as f64,
            count_normal: bm.count_circles as u64,
            count_slider: bm.count_sliders as u64,
            count_spinner: bm.count_sliders as u64,
            max_combo: bm.max_combo.map(|v| v as u64),
            bpm: bm.bpm as f64,
            drain_length: Duration::from_secs(bm.seconds_drain as u64),
            total_length: Duration::from_secs(bm.seconds_total as u64),
        }
    }
}

impl From<rosu::GameMode> for Mode {
    fn from(value: rosu::GameMode) -> Self {
        match value {
            rosu::GameMode::Osu => Mode::Std,
            rosu::GameMode::Taiko => Mode::Taiko,
            rosu::GameMode::Catch => Mode::Catch,
            rosu::GameMode::Mania => Mode::Mania,
        }
    }
}

impl From<Mode> for rosu::GameMode {
    fn from(value: Mode) -> Self {
        match value {
            Mode::Std => rosu::GameMode::Osu,
            Mode::Taiko => rosu::GameMode::Taiko,
            Mode::Catch => rosu::GameMode::Catch,
            Mode::Mania => rosu::GameMode::Mania,
        }
    }
}

impl From<rosu::beatmap::Genre> for Genre {
    fn from(value: rosu::beatmap::Genre) -> Self {
        match value {
            rosu::beatmap::Genre::Any => Genre::Any,
            rosu::beatmap::Genre::Unspecified => Genre::Unspecified,
            rosu::beatmap::Genre::VideoGame => Genre::VideoGame,
            rosu::beatmap::Genre::Anime => Genre::Anime,
            rosu::beatmap::Genre::Rock => Genre::Rock,
            rosu::beatmap::Genre::Pop => Genre::Pop,
            rosu::beatmap::Genre::Other => Genre::Other,
            rosu::beatmap::Genre::Novelty => Genre::Novelty,
            rosu::beatmap::Genre::HipHop => Genre::HipHop,
            rosu::beatmap::Genre::Electronic => Genre::Electronic,
            rosu::beatmap::Genre::Metal => Genre::Metal,
            rosu::beatmap::Genre::Classical => Genre::Classical,
            rosu::beatmap::Genre::Folk => Genre::Folk,
            rosu::beatmap::Genre::Jazz => Genre::Jazz,
        }
    }
}

impl From<rosu::beatmap::Language> for Language {
    fn from(value: rosu::beatmap::Language) -> Self {
        match value {
            rosu::beatmap::Language::Any => Language::Any,
            rosu::beatmap::Language::Other => Language::Other,
            rosu::beatmap::Language::English => Language::English,
            rosu::beatmap::Language::Japanese => Language::Japanese,
            rosu::beatmap::Language::Chinese => Language::Chinese,
            rosu::beatmap::Language::Instrumental => Language::Instrumental,
            rosu::beatmap::Language::Korean => Language::Korean,
            rosu::beatmap::Language::French => Language::French,
            rosu::beatmap::Language::German => Language::German,
            rosu::beatmap::Language::Swedish => Language::Swedish,
            rosu::beatmap::Language::Spanish => Language::Spanish,
            rosu::beatmap::Language::Italian => Language::Italian,
            rosu::beatmap::Language::Russian => Language::Russian,
            rosu::beatmap::Language::Polish => Language::Polish,
            rosu::beatmap::Language::Unspecified => Language::Unspecified,
        }
    }
}

impl From<rosu::Grade> for Rank {
    fn from(value: rosu::Grade) -> Self {
        match value {
            rosu::Grade::F => Rank::F,
            rosu::Grade::D => Rank::D,
            rosu::Grade::C => Rank::C,
            rosu::Grade::B => Rank::B,
            rosu::Grade::A => Rank::A,
            rosu::Grade::S => Rank::S,
            rosu::Grade::SH => Rank::SH,
            rosu::Grade::X => Rank::SS,
            rosu::Grade::XH => Rank::SSH,
        }
    }
}

// impl From<Mods> for rosu::mods::GameModsIntermode {
//     fn from(value: Mods) -> Self {
//         let mut res = GameModsIntermode::new();
//         const MOD_MAP: &[(Mods, GameModIntermode)] = &[
//             (Mods::NF, GameModIntermode::NoFail),
//             (Mods::EZ, GameModIntermode::Easy),
//             (Mods::TD, GameModIntermode::TouchDevice),
//             (Mods::HD, GameModIntermode::Hidden),
//             (Mods::HR, GameModIntermode::HardRock),
//             (Mods::SD, GameModIntermode::SuddenDeath),
//             (Mods::DT, GameModIntermode::DoubleTime),
//             (Mods::RX, GameModIntermode::Relax),
//             (Mods::HT, GameModIntermode::HalfTime),
//             (Mods::NC, GameModIntermode::Nightcore),
//             (Mods::FL, GameModIntermode::Flashlight),
//             (Mods::AT, GameModIntermode::Autoplay),
//             (Mods::SO, GameModIntermode::SpunOut),
//             (Mods::AP, GameModIntermode::Autopilot),
//             (Mods::PF, GameModIntermode::Perfect),
//             (Mods::KEY1, GameModIntermode::OneKey),
//             (Mods::KEY2, GameModIntermode::TwoKeys),
//             (Mods::KEY3, GameModIntermode::ThreeKeys),
//             (Mods::KEY4, GameModIntermode::FourKeys),
//             (Mods::KEY5, GameModIntermode::FiveKeys),
//             (Mods::KEY6, GameModIntermode::SixKeys),
//             (Mods::KEY7, GameModIntermode::SevenKeys),
//             (Mods::KEY8, GameModIntermode::EightKeys),
//             (Mods::KEY9, GameModIntermode::NineKeys),
//         ];
//         for (m1, m2) in MOD_MAP {
//             if value.contains(*m1) {
//                 res.insert(*m2);
//             }
//         }
//         if !value.contains(Mods::LAZER) {
//             res.insert(GameModIntermode::Classic);
//         }
//         res
//     }
// }

// impl From<rosu::mods::GameModsIntermode> for Mods {
//     fn from(value: rosu_v2::prelude::GameModsIntermode) -> Self {
//         let init = if value.contains(GameModIntermode::Classic) {
//             Mods::NOMOD
//         } else {
//             Mods::LAZER
//         };
//         value
//             .into_iter()
//             .map(|m| match m {
//                 GameModIntermode::NoFail => Mods::NF,
//                 GameModIntermode::Easy => Mods::EZ,
//                 GameModIntermode::TouchDevice => Mods::TD,
//                 GameModIntermode::Hidden => Mods::HD,
//                 GameModIntermode::HardRock => Mods::HR,
//                 GameModIntermode::SuddenDeath => Mods::SD,
//                 GameModIntermode::DoubleTime => Mods::DT,
//                 GameModIntermode::Relax => Mods::RX,
//                 GameModIntermode::HalfTime => Mods::HT,
//                 GameModIntermode::Nightcore => Mods::DT | Mods::NC,
//                 GameModIntermode::Flashlight => Mods::FL,
//                 GameModIntermode::Autoplay => Mods::AT,
//                 GameModIntermode::SpunOut => Mods::SO,
//                 GameModIntermode::Autopilot => Mods::AP,
//                 GameModIntermode::Perfect => Mods::SD | Mods::PF,
//                 GameModIntermode::OneKey => Mods::KEY1,
//                 GameModIntermode::TwoKeys => Mods::KEY2,
//                 GameModIntermode::ThreeKeys => Mods::KEY3,
//                 GameModIntermode::FourKeys => Mods::KEY4,
//                 GameModIntermode::FiveKeys => Mods::KEY5,
//                 GameModIntermode::SixKeys => Mods::KEY6,
//                 GameModIntermode::SevenKeys => Mods::KEY7,
//                 GameModIntermode::EightKeys => Mods::KEY8,
//                 GameModIntermode::NineKeys => Mods::KEY9,
//                 GameModIntermode::Classic => Mods::NOMOD,
//                 _ => Mods::UNKNOWN,
//             })
//             .fold(init, |a, b| a | b)

//         // Mods::from_bits_truncate(value.bits() as u64)
//     }
// }

// impl From<rosu::mods::GameMods> for Mods {
//     fn from(value: rosu::mods::GameMods) -> Self {
//         let unknown =
//             rosu::mods::GameModIntermode::Unknown(rosu_v2::prelude::UnknownMod::default());
//         value
//             .iter()
//             .cloned()
//             .map(|m| match m {
//                 rosu::mods::GameMod::HalfTimeOsu(ht)
//                     if ht.speed_change.is_some_and(|v| v != 0.75) =>
//                 {
//                     unknown
//                 }
//                 rosu::mods::GameMod::DaycoreOsu(dc)
//                     if dc.speed_change.is_some_and(|v| v != 0.75) =>
//                 {
//                     unknown
//                 }
//                 rosu::mods::GameMod::DaycoreOsu(_) => rosu::mods::GameModIntermode::HalfTime,
//                 rosu::mods::GameMod::DoubleTimeOsu(dt)
//                     if dt.speed_change.is_some_and(|v| v != 1.5) =>
//                 {
//                     unknown
//                 }
//                 rosu::mods::GameMod::NightcoreOsu(nc)
//                     if nc.speed_change.is_some_and(|v| v != 1.5) =>
//                 {
//                     unknown
//                 }
//                 rosu::mods::GameMod::HalfTimeTaiko(ht)
//                     if ht.speed_change.is_some_and(|v| v != 0.75) =>
//                 {
//                     unknown
//                 }
//                 rosu::mods::GameMod::DaycoreTaiko(dc)
//                     if dc.speed_change.is_some_and(|v| v != 0.75) =>
//                 {
//                     unknown
//                 }
//                 rosu::mods::GameMod::DaycoreTaiko(_) => rosu::mods::GameModIntermode::HalfTime,
//                 rosu::mods::GameMod::DoubleTimeTaiko(dt)
//                     if dt.speed_change.is_some_and(|v| v != 1.5) =>
//                 {
//                     unknown
//                 }
//                 rosu::mods::GameMod::NightcoreTaiko(nc)
//                     if nc.speed_change.is_some_and(|v| v != 1.5) =>
//                 {
//                     unknown
//                 }
//                 rosu::mods::GameMod::HalfTimeCatch(ht)
//                     if ht.speed_change.is_some_and(|v| v != 0.75) =>
//                 {
//                     unknown
//                 }
//                 rosu::mods::GameMod::DaycoreCatch(dc)
//                     if dc.speed_change.is_some_and(|v| v != 0.75) =>
//                 {
//                     unknown
//                 }
//                 rosu::mods::GameMod::DaycoreCatch(_) => rosu::mods::GameModIntermode::HalfTime,
//                 rosu::mods::GameMod::DoubleTimeCatch(dt)
//                     if dt.speed_change.is_some_and(|v| v != 1.5) =>
//                 {
//                     unknown
//                 }
//                 rosu::mods::GameMod::NightcoreCatch(nc)
//                     if nc.speed_change.is_some_and(|v| v != 1.5) =>
//                 {
//                     unknown
//                 }
//                 rosu::mods::GameMod::HalfTimeMania(ht)
//                     if ht.speed_change.is_some_and(|v| v != 0.75) =>
//                 {
//                     unknown
//                 }
//                 rosu::mods::GameMod::DaycoreMania(dc)
//                     if dc.speed_change.is_some_and(|v| v != 0.75) =>
//                 {
//                     unknown
//                 }
//                 rosu::mods::GameMod::DaycoreMania(_) => rosu::mods::GameModIntermode::HalfTime,
//                 rosu::mods::GameMod::DoubleTimeMania(dt)
//                     if dt.speed_change.is_some_and(|v| v != 1.5) =>
//                 {
//                     unknown
//                 }
//                 _ => m.intermode(),
//             })
//             .collect::<GameModsIntermode>()
//             .into()
//     }
// }
