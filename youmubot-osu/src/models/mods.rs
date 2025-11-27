use regex::Regex;
use rosu::GameMods;
use rosu_v2::model::mods as rosu;
use rosu_v2::prelude::GameModsIntermode;
use std::borrow::Cow;
use std::fmt;
use std::fmt::Write;
use std::str::FromStr;
use youmubot_prelude::*;

use crate::Mode;

// const LAZER_TEXT: &str = "v2";

lazy_static::lazy_static! {
    // Beatmap(set) hooks
    static ref MODS: Regex = Regex::new(
        // r"(?:https?://)?osu\.ppy\.sh/(?P<link_type>s|b|beatmaps)/(?P<id>\d+)(?:[\&\?]m=(?P<mode>[0123]))?(?:\+(?P<mods>[A-Z]+))?"
        r"^((\+?)(?P<mods>([A-Za-z0-9][A-Za-z])+))?(@(?P<clock>\d(\.\d+)?)x)?(?P<stats>(@(ar|AR|od|OD|cs|CS|hp|HP)\d(\.\d)?)+)?(?P<lazer>v2)?$"
    ).unwrap();
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct UnparsedMods {
    mods: Cow<'static, str>,
    is_lazer: bool, // not really used
    clock: Option<f64>,
}

#[derive(thiserror::Error, Debug)]
pub enum ModParseError {
    #[error("invalid mods `{0}`")]
    Invalid(String),
    #[error("not a mod: `{0}`")]
    NotAMod(String),
}

impl FromStr for UnparsedMods {
    type Err = ModParseError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        if s.is_empty() {
            return Ok(UnparsedMods::default());
        }
        let ms = match MODS.captures(s) {
            Some(m) => m,
            None => return Err(ModParseError::Invalid(s.to_owned())),
        };
        let mods = ms.name("mods").map(|v| v.as_str().to_owned());
        let imods = match &mods {
            Some(mods) => Some(match GameModsIntermode::try_from_acronyms(mods) {
                None => return Err(ModParseError::NotAMod(mods.to_owned())),
                Some(mods) => mods,
            }),
            None => None,
        };
        let is_lazer = ms.name("lazer").is_some()
            || imods.is_none_or(|m| !m.contains_acronym("CL".parse().unwrap()));
        Ok(Self {
            mods: mods.map(|v| v.into()).unwrap_or("".into()),
            is_lazer,
            clock: ms
                .name("clock")
                .map(|v| v.as_str().parse::<_>().unwrap())
                .filter(|v| *v > 0.0),
        })
    }
}

impl UnparsedMods {
    /// Convert to [Mods].
    pub fn to_mods(&self, mode: Mode) -> Result<Mods> {
        use rosu_v2::prelude::*;
        let mut mods = Mods::from_str(&self.mods, mode)?;
        if let Some(clock) = self.clock {
            let has_night_day_core = mods.inner.contains_intermode(GameModIntermode::Nightcore)
                || mods.inner.contains_intermode(GameModIntermode::Daycore);
            mods.inner.remove_all_intermode([
                GameModIntermode::Daycore,
                GameModIntermode::Nightcore,
                GameModIntermode::DoubleTime,
                GameModIntermode::HalfTime,
            ]);
            let mut speed_change = Some(clock);
            let adjust_pitch: Option<bool> = None;
            if clock < 1.0 {
                speed_change = speed_change.filter(|v| *v != 0.75);
                mods.inner.insert(if has_night_day_core {
                    match mode {
                        Mode::Std => GameMod::DaycoreOsu(DaycoreOsu { speed_change }),
                        Mode::Taiko => GameMod::DaycoreTaiko(DaycoreTaiko { speed_change }),
                        Mode::Catch => GameMod::DaycoreCatch(DaycoreCatch { speed_change }),
                        Mode::Mania => GameMod::DaycoreMania(DaycoreMania { speed_change }),
                    }
                } else {
                    match mode {
                        Mode::Std => GameMod::HalfTimeOsu(HalfTimeOsu {
                            speed_change,
                            adjust_pitch,
                        }),
                        Mode::Taiko => GameMod::HalfTimeTaiko(HalfTimeTaiko {
                            speed_change,
                            adjust_pitch,
                        }),
                        Mode::Catch => GameMod::HalfTimeCatch(HalfTimeCatch {
                            speed_change,
                            adjust_pitch,
                        }),
                        Mode::Mania => GameMod::HalfTimeMania(HalfTimeMania {
                            speed_change,
                            adjust_pitch,
                        }),
                    }
                })
            }
            if clock > 1.0 {
                speed_change = speed_change.filter(|v| *v != 1.5);
                mods.inner.insert(if has_night_day_core {
                    match mode {
                        Mode::Std => GameMod::NightcoreOsu(NightcoreOsu { speed_change }),
                        Mode::Taiko => GameMod::NightcoreTaiko(NightcoreTaiko { speed_change }),
                        Mode::Catch => GameMod::NightcoreCatch(NightcoreCatch { speed_change }),
                        Mode::Mania => GameMod::NightcoreMania(NightcoreMania { speed_change }),
                    }
                } else {
                    match mode {
                        Mode::Std => GameMod::DoubleTimeOsu(DoubleTimeOsu {
                            speed_change,
                            adjust_pitch,
                        }),
                        Mode::Taiko => GameMod::DoubleTimeTaiko(DoubleTimeTaiko {
                            speed_change,
                            adjust_pitch,
                        }),
                        Mode::Catch => GameMod::DoubleTimeCatch(DoubleTimeCatch {
                            speed_change,
                            adjust_pitch,
                        }),
                        Mode::Mania => GameMod::DoubleTimeMania(DoubleTimeMania {
                            speed_change,
                            adjust_pitch,
                        }),
                    }
                })
            }
        };
        Ok(mods)
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Mods {
    pub inner: GameMods,
}

/// Store overrides to the stats.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Stats {
    pub ar: Option<f64>,
    pub od: Option<f64>,
    pub hp: Option<f64>,
    pub cs: Option<f64>,
}

impl Stats {
    pub fn from_option(ar: Option<f64>, od: Option<f64>, hp: Option<f64>, cs: Option<f64>) -> Self {
        Self { ar, od, hp, cs }
    }

    pub fn has_any(&self) -> bool {
        self.ar.is_some() || self.od.is_some() || self.hp.is_some() || self.cs.is_some()
    }
}

impl Mods {
    pub const NOMOD: &'static Mods = &Mods {
        inner: GameMods::new(),
    };

    pub fn is_lazer(&self) -> bool {
        !self
            .inner
            .contains_intermode(rosu::GameModIntermode::Classic)
    }

    // fn classic_mod_of(mode: Mode) -> rosu::GameMod {
    //     match mode {
    //         Mode::Std => rosu::GameMod::ClassicOsu(rosu::generated_mods::ClassicOsu::default()),
    //         Mode::Taiko => {
    //             rosu::GameMod::ClassicTaiko(rosu::generated_mods::ClassicTaiko::default())
    //         }
    //         Mode::Catch => {
    //             rosu::GameMod::ClassicCatch(rosu::generated_mods::ClassicCatch::default())
    //         }
    //         Mode::Mania => {
    //             rosu::GameMod::ClassicMania(rosu::generated_mods::ClassicMania::default())
    //         }
    //     }
    // }

    pub fn overrides(&self) -> Stats {
        use rosu_v2::prelude::GameMod::*;
        self.inner
            .iter()
            .find_map(|m| {
                Some(match m {
                    DifficultyAdjustOsu(da) => Stats::from_option(
                        da.approach_rate,
                        da.overall_difficulty,
                        da.drain_rate,
                        da.circle_size,
                    ),
                    DifficultyAdjustTaiko(da) => {
                        Stats::from_option(None, da.overall_difficulty, da.drain_rate, None)
                    }
                    DifficultyAdjustCatch(da) => Stats::from_option(
                        da.approach_rate,
                        da.overall_difficulty,
                        da.drain_rate,
                        da.circle_size,
                    ),
                    DifficultyAdjustMania(da) => {
                        Stats::from_option(None, da.overall_difficulty, da.drain_rate, None)
                    }
                    _ => return None,
                })
            })
            .unwrap_or_default()
    }
}

// impl From<GameMods> for Mods {
//     fn from(inner: GameMods) -> Self {
//         Self { inner }
//     }
// }

// bitflags::bitflags! {
//     /// The mods available to osu!
//     #[derive(std::default::Default, Serialize, Deserialize)]
//     pub struct Mods: u64 {
//         const NF = 1 << 0;
//         const EZ = 1 << 1;
//         const TD = 1 << 2;
//         const HD = 1 << 3;
//         const HR = 1 << 4;
//         const SD = 1 << 5;
//         const DT = 1 << 6;
//         const RX = 1 << 7;
//         const HT = 1 << 8;
//         const NC = 1 << 9;
//         const FL = 1 << 10;
//         const AT = 1 << 11;
//         const SO = 1 << 12;
//         const AP = 1 << 13;
//         const PF = 1 << 14;
//         const KEY4 = 1 << 15; /* TODO: what are these abbreviated to? */
//         const KEY5 = 1 << 16;
//         const KEY6 = 1 << 17;
//         const KEY7 = 1 << 18;
//         const KEY8 = 1 << 19;
//         const FADEIN = 1 << 20;
//         const RANDOM = 1 << 21;
//         const CINEMA = 1 << 22;
//         const TARGET = 1 << 23;
//         const KEY9 = 1 << 24;
//         const KEYCOOP = 1 << 25;
//         const KEY1 = 1 << 26;
//         const KEY3 = 1 << 27;
//         const KEY2 = 1 << 28;
//         const SCOREV2 = 1 << 29;

//         // Made up flags
//         const LAZER = 1 << 59;
//         const UNKNOWN = 1 << 60;
//     }
// }

// impl Mods {
//     pub const NOMOD: Mods = Mods::empty();
//     pub const TOUCH_DEVICE: Mods = Self::TD;
//     pub const NOVIDEO: Mods = Self::TD; /* never forget */
//     pub const SPEED_CHANGING: Mods =
//         Mods::from_bits_truncate(Self::DT.bits | Self::HT.bits | Self::NC.bits);
//     pub const MAP_CHANGING: Mods =
//         Mods::from_bits_truncate(Self::HR.bits | Self::EZ.bits | Self::SPEED_CHANGING.bits);
// }

// const MODS_WITH_NAMES: &[(Mods, &str)] = &[
//     (Mods::NF, "NF"),
//     (Mods::EZ, "EZ"),
//     (Mods::TD, "TD"),
//     (Mods::HD, "HD"),
//     (Mods::HR, "HR"),
//     (Mods::SD, "SD"),
//     (Mods::DT, "DT"),
//     (Mods::RX, "RX"),
//     (Mods::HT, "HT"),
//     (Mods::NC, "NC"),
//     (Mods::FL, "FL"),
//     (Mods::AT, "AT"),
//     (Mods::SO, "SO"),
//     (Mods::AP, "AP"),
//     (Mods::PF, "PF"),
//     (Mods::KEY1, "1K"),
//     (Mods::KEY2, "2K"),
//     (Mods::KEY3, "3K"),
//     (Mods::KEY4, "4K"),
//     (Mods::KEY5, "5K"),
//     (Mods::KEY6, "6K"),
//     (Mods::KEY7, "7K"),
//     (Mods::KEY8, "8K"),
//     (Mods::KEY9, "9K"),
//     (Mods::UNKNOWN, "??"),
// ];

impl Mods {
    pub fn bits(&self) -> u32 {
        self.inner.bits()
    }

    pub fn contains(&self, other: &Mods) -> bool {
        other.inner.iter().all(|m| self.inner.contains(m))
    }
    // Format the mods into a string with padded size.
    pub fn to_string_padded(&self, size: usize) -> String {
        let s = format!("{}", self);
        let real_padded = size;
        format!("{:>mw$}", s, mw = real_padded)
    }

    /// Get details on the mods, if they are present.
    pub fn details(&self) -> Vec<String> {
        use rosu::GameMod::*;
        fn fmt_speed_change(
            mod_name: &str,
            speed_change: &Option<f64>,
            adjust_pitch: &Option<bool>,
        ) -> Option<String> {
            if speed_change.is_none() && adjust_pitch.is_none() {
                return None;
            }
            let mut s = format!("**{}**: ", mod_name);
            let mut need_comma = false;
            if let Some(speed) = speed_change {
                s += &format!("speed **{:.2}x**", speed);
                need_comma = true;
            }
            if let Some(true) = adjust_pitch {
                if need_comma {
                    s += ", ";
                }
                s += "pitch **changed**"
            }
            Some(s)
        }
        fn fmt_diff_adj(
            ar: Option<f64>,
            od: Option<f64>,
            hp: Option<f64>,
            cs: Option<f64>,
        ) -> Option<String> {
            let stats = [("AR", ar), ("OD", od), ("HP", hp), ("CS", cs)];
            let mut output = String::with_capacity(4 * (2 + 3 + 4) + 3 * 2);
            for (name, stat) in stats {
                if let Some(stat) = stat {
                    if !output.is_empty() {
                        write!(output, ", ").unwrap();
                    }
                    write!(output, "{}**{:.1}**", name, stat).unwrap();
                }
            }
            if output.is_empty() {
                None
            } else {
                Some("**DA**: ".to_owned() + &output)
            }
        }
        self.inner
            .iter()
            .filter_map(|m| match m {
                DoubleTimeOsu(dt) => fmt_speed_change("DT", &dt.speed_change, &dt.adjust_pitch),
                DoubleTimeTaiko(dt) => fmt_speed_change("DT", &dt.speed_change, &dt.adjust_pitch),
                DoubleTimeCatch(dt) => fmt_speed_change("DT", &dt.speed_change, &dt.adjust_pitch),
                DoubleTimeMania(dt) => fmt_speed_change("DT", &dt.speed_change, &dt.adjust_pitch),
                NightcoreOsu(dt) => fmt_speed_change("NC", &dt.speed_change, &None),
                NightcoreTaiko(dt) => fmt_speed_change("NC", &dt.speed_change, &None),
                NightcoreCatch(dt) => fmt_speed_change("NC", &dt.speed_change, &None),
                NightcoreMania(dt) => fmt_speed_change("NC", &dt.speed_change, &None),
                HalfTimeOsu(ht) => fmt_speed_change("HT", &ht.speed_change, &ht.adjust_pitch),
                HalfTimeTaiko(ht) => fmt_speed_change("HT", &ht.speed_change, &ht.adjust_pitch),
                HalfTimeCatch(ht) => fmt_speed_change("HT", &ht.speed_change, &ht.adjust_pitch),
                HalfTimeMania(ht) => fmt_speed_change("HT", &ht.speed_change, &ht.adjust_pitch),
                DaycoreOsu(ht) => fmt_speed_change("DC", &ht.speed_change, &None),
                DaycoreTaiko(ht) => fmt_speed_change("DC", &ht.speed_change, &None),
                DaycoreCatch(ht) => fmt_speed_change("DC", &ht.speed_change, &None),
                DaycoreMania(ht) => fmt_speed_change("DC", &ht.speed_change, &None),

                DifficultyAdjustOsu(da) => fmt_diff_adj(
                    da.approach_rate,
                    da.overall_difficulty,
                    da.drain_rate,
                    da.circle_size,
                ),
                DifficultyAdjustTaiko(da) => {
                    fmt_diff_adj(None, da.overall_difficulty, da.drain_rate, None)
                }
                DifficultyAdjustCatch(da) => fmt_diff_adj(
                    da.approach_rate,
                    da.overall_difficulty,
                    da.drain_rate,
                    da.circle_size,
                ),
                DifficultyAdjustMania(da) => {
                    fmt_diff_adj(None, da.overall_difficulty, da.drain_rate, None)
                }
                _ => None,
            })
            .collect()
    }
}

impl Mods {
    pub fn from_gamemods(mods: GameMods) -> Self {
        Self { inner: mods }
    }
    pub fn from_str(mut s: &str, mode: Mode) -> Result<Self> {
        // Strip leading +
        if s.starts_with('+') {
            s = &s[1..];
        }
        let intermode =
            GameModsIntermode::try_from_acronyms(s).ok_or_else(|| error!("Invalid mods: {}", s))?;
        let inner = intermode
            .try_with_mode(mode.into())
            .ok_or_else(|| error!("Invalid mods for `{}`: {}", mode, intermode))?;
        // Always add classic mod to `inner`
        // inner.insert(Self::classic_mod_of(mode));
        if !inner.is_valid() {
            return Err(error!("Incompatible mods found: {}", inner));
        }
        Ok(Self::from_gamemods(inner))
        // let mut res = GameModsIntermode::default();
        // while s.len() >= 2 {
        //     let (m, nw) = s.split_at(2);
        //     s = nw;
        //     match &m.to_uppercase()[..] {
        //         "NF" => res.insert(GameModIntermode::NoFail),
        //         "EZ" => res.insert(GameModIntermode::Easy),
        //         "TD" => res.insert(GameModIntermode::TouchDevice),
        //         "HD" => res.insert(GameModIntermode::Hidden),
        //         "HR" => res.insert(GameModIntermode::HardRock),
        //         "SD" => res.insert(GameModIntermode::SuddenDeath),
        //         "DT" => res.insert(GameModIntermode::DoubleTime),
        //         "RX" => res.insert(GameModIntermode::Relax),
        //         "HT" => res.insert(GameModIntermode::HalfTime),
        //         "NC" => res.insert(GameModIntermode::Nightcore),
        //         "FL" => res.insert(GameModIntermode::Flashlight),
        //         "AT" => res.insert(GameModIntermode::Autopilot),
        //         "SO" => res.insert(GameModIntermode::SpunOut),
        //         "AP" => res.insert(GameModIntermode::Autoplay),
        //         "PF" => res.insert(GameModIntermode::Perfect),
        //         "1K" => res.insert(GameModIntermode::OneKey),
        //         "2K" => res.insert(GameModIntermode::TwoKeys),
        //         "3K" => res.insert(GameModIntermode::ThreeKeys),
        //         "4K" => res.insert(GameModIntermode::FourKeys),
        //         "5K" => res.insert(GameModIntermode::FiveKeys),
        //         "6K" => res.insert(GameModIntermode::SixKeys),
        //         "7K" => res.insert(GameModIntermode::SevenKeys),
        //         "8K" => res.insert(GameModIntermode::EightKeys),
        //         "9K" => res.insert(GameModIntermode::NineKeys),
        //         v => return Err(format!("{} is not a valid mod", v)),
        //     }
        // }
        // if !s.is_empty() {
        //     Err("String of odd length is not a mod string".to_owned())
        // } else {
        //     Ok(Mods { inner: res })
        // }
    }
}

impl fmt::Display for Mods {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mods = &self.inner;
        if !mods.is_empty() {
            write!(f, "+{}", mods)?;
        }
        if let Some(clock) = mods.clock_rate() {
            if clock != 1.0 && clock != 1.5 && clock != 0.75 {
                write!(f, "@{:.2}x", clock)?;
            }
        }
        Ok(())
        // if !(*self & (Mods::all() ^ Mods::LAZER)).is_empty() {
        //     write!(f, "+")?;
        //     for p in MODS_WITH_NAMES.iter() {
        //         if !self.contains(p.0) {
        //             continue;
        //         }
        //         match p.0 {
        //             Mods::DT if self.contains(Mods::NC) => continue,
        //             Mods::SD if self.contains(Mods::PF) => continue,
        //             _ => (),
        //         };
        //         write!(f, "{}", p.1)?;
        //     }
        // }
        // if self.contains(Mods::LAZER) {
        //     write!(f, "{}", LAZER_TEXT)?;
        // }
        // Ok(())
    }
}
