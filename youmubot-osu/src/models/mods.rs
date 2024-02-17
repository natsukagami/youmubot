use serde::{Deserialize, Serialize};
use std::fmt;

const LAZER_TEXT: &str = "v2";

bitflags::bitflags! {
    /// The mods available to osu!
    #[derive(std::default::Default, Serialize, Deserialize)]
    pub struct Mods: u64 {
        const NF = 1 << 0;
        const EZ = 1 << 1;
        const TD = 1 << 2;
        const HD = 1 << 3;
        const HR = 1 << 4;
        const SD = 1 << 5;
        const DT = 1 << 6;
        const RX = 1 << 7;
        const HT = 1 << 8;
        const NC = 1 << 9;
        const FL = 1 << 10;
        const AT = 1 << 11;
        const SO = 1 << 12;
        const AP = 1 << 13;
        const PF = 1 << 14;
        const KEY4 = 1 << 15; /* TODO: what are these abbreviated to? */
        const KEY5 = 1 << 16;
        const KEY6 = 1 << 17;
        const KEY7 = 1 << 18;
        const KEY8 = 1 << 19;
        const FADEIN = 1 << 20;
        const RANDOM = 1 << 21;
        const CINEMA = 1 << 22;
        const TARGET = 1 << 23;
        const KEY9 = 1 << 24;
        const KEYCOOP = 1 << 25;
        const KEY1 = 1 << 26;
        const KEY3 = 1 << 27;
        const KEY2 = 1 << 28;
        const SCOREV2 = 1 << 29;

        // Made up flags
        const LAZER = 1 << 59;
        const UNKNOWN = 1 << 60;
    }
}

impl Mods {
    pub const NOMOD: Mods = Mods::empty();
    pub const TOUCH_DEVICE: Mods = Self::TD;
    pub const NOVIDEO: Mods = Self::TD; /* never forget */
    pub const SPEED_CHANGING: Mods =
        Mods::from_bits_truncate(Self::DT.bits | Self::HT.bits | Self::NC.bits);
    pub const MAP_CHANGING: Mods =
        Mods::from_bits_truncate(Self::HR.bits | Self::EZ.bits | Self::SPEED_CHANGING.bits);
}

const MODS_WITH_NAMES: &[(Mods, &str)] = &[
    (Mods::NF, "NF"),
    (Mods::EZ, "EZ"),
    (Mods::TD, "TD"),
    (Mods::HD, "HD"),
    (Mods::HR, "HR"),
    (Mods::SD, "SD"),
    (Mods::DT, "DT"),
    (Mods::RX, "RX"),
    (Mods::HT, "HT"),
    (Mods::NC, "NC"),
    (Mods::FL, "FL"),
    (Mods::AT, "AT"),
    (Mods::SO, "SO"),
    (Mods::AP, "AP"),
    (Mods::PF, "PF"),
    (Mods::KEY1, "1K"),
    (Mods::KEY2, "2K"),
    (Mods::KEY3, "3K"),
    (Mods::KEY4, "4K"),
    (Mods::KEY5, "5K"),
    (Mods::KEY6, "6K"),
    (Mods::KEY7, "7K"),
    (Mods::KEY8, "8K"),
    (Mods::KEY9, "9K"),
    (Mods::UNKNOWN, "??"),
];

impl Mods {
    // Return the string length of the string representation of the mods.
    pub fn str_len(&self) -> usize {
        let s = format!("{}", self);
        s.len()
    }

    // Format the mods into a string with padded size.
    pub fn to_string_padded(&self, size: usize) -> String {
        let s = format!("{}", self);
        let real_padded = size;
        format!("{:>mw$}", s, mw = real_padded)
    }
}

impl std::str::FromStr for Mods {
    type Err = String;
    fn from_str(mut s: &str) -> Result<Self, Self::Err> {
        let mut res = Self::default();
        // Strip leading +
        if s.starts_with('+') {
            s = &s[1..];
        }
        while s.len() >= 2 {
            let (m, nw) = s.split_at(2);
            s = nw;
            match &m.to_uppercase()[..] {
                "NF" => res |= Mods::NF,
                "EZ" => res |= Mods::EZ,
                "TD" => res |= Mods::TD,
                "HD" => res |= Mods::HD,
                "HR" => res |= Mods::HR,
                "SD" => res |= Mods::SD,
                "DT" => res |= Mods::DT,
                "RX" => res |= Mods::RX,
                "HT" => res |= Mods::HT,
                "NC" => res |= Mods::NC | Mods::DT,
                "FL" => res |= Mods::FL,
                "AT" => res |= Mods::AT,
                "SO" => res |= Mods::SO,
                "AP" => res |= Mods::AP,
                "PF" => res |= Mods::PF,
                "1K" => res |= Mods::KEY1,
                "2K" => res |= Mods::KEY2,
                "3K" => res |= Mods::KEY3,
                "4K" => res |= Mods::KEY4,
                "5K" => res |= Mods::KEY5,
                "6K" => res |= Mods::KEY6,
                "7K" => res |= Mods::KEY7,
                "8K" => res |= Mods::KEY8,
                "9K" => res |= Mods::KEY9,
                "??" => res |= Mods::UNKNOWN,
                v => return Err(format!("{} is not a valid mod", v)),
            }
        }
        if !s.is_empty() {
            Err("String of odd length is not a mod string".to_owned())
        } else {
            Ok(res)
        }
    }
}

impl fmt::Display for Mods {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if !(*self & (Mods::all() ^ Mods::LAZER)).is_empty() {
            write!(f, "+")?;
            for p in MODS_WITH_NAMES.iter() {
                if !self.contains(p.0) {
                    continue;
                }
                match p.0 {
                    Mods::DT if self.contains(Mods::NC) => continue,
                    Mods::SD if self.contains(Mods::PF) => continue,
                    _ => (),
                };
                write!(f, "{}", p.1)?;
            }
        }
        if self.contains(Mods::LAZER) {
            write!(f, "{}", LAZER_TEXT)?;
        }
        Ok(())
    }
}
