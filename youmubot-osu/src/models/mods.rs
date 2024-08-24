use rosu::{GameModIntermode, GameMods};
use rosu_v2::model::mods as rosu;
use rosu_v2::prelude::GameModsIntermode;
use std::borrow::Cow;
use std::fmt;
use std::str::FromStr;
use youmubot_prelude::*;

use crate::Mode;

const LAZER_TEXT: &str = "v2";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnparsedMods(Cow<'static, str>);

impl Default for UnparsedMods {
    fn default() -> Self {
        Self("".into())
    }
}

impl FromStr for UnparsedMods {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        if s == "" {
            return Ok(UnparsedMods("".into()));
        }
        let v = match s.strip_prefix("+") {
            Some(v) => v,
            None => return Err(format!("Mods need to start with +: {}", s)),
        };
        if v.chars().all(|c| c.is_digit(10) || c.is_ascii_uppercase()) {
            Ok(Self(v.to_owned().into()))
        } else {
            Err(format!("Not a mod string: {}", s))
        }
    }
}

impl UnparsedMods {
    /// Convert to [Mods].
    pub fn to_mods(&self, mode: Mode) -> Result<Mods> {
        Mods::from_str(&self.0, mode)
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Mods {
    pub inner: GameMods,
}

impl Mods {
    pub const NOMOD: &'static Mods = &Mods {
        inner: GameMods::new(),
    };
}

impl From<GameMods> for Mods {
    fn from(inner: GameMods) -> Self {
        Self { inner }
    }
}

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
}

impl Mods {
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
        Ok(Self { inner })
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
        let is_lazer = !self.inner.contains_intermode(GameModIntermode::Classic);
        let mods = if !is_lazer {
            let mut v = self.inner.clone();
            v.remove_intermode(GameModIntermode::Classic);
            Cow::Owned(v)
        } else {
            Cow::Borrowed(&self.inner)
        };
        if !mods.is_empty() {
            write!(f, "+{}", mods)?;
        }
        if is_lazer {
            write!(f, "{}", LAZER_TEXT)?;
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
