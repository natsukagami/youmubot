pub use duration::Duration;
pub use username_arg::UsernameArg;

mod duration {
    use crate::{Error, Result};
    use std::fmt;
    use std::time::Duration as StdDuration;

    const INVALID_DURATION: &str = "Not a valid duration";

    /// Parse a single duration unit
    fn parse_duration_string(s: &str) -> Result<StdDuration> {
        // We reject the empty case
        if s.is_empty() {
            return Err(Error::msg("empty strings are not valid durations"));
        }
        struct ParseStep {
            current_value: Option<u64>,
            current_duration: StdDuration,
        }
        s.chars()
            .try_fold(
                ParseStep {
                    current_value: None,
                    current_duration: StdDuration::from_secs(0),
                },
                |s, item| match (item, s.current_value) {
                    ('0'..='9', v) => Ok(ParseStep {
                        current_value: Some(v.unwrap_or(0) * 10 + ((item as u64) - ('0' as u64))),
                        ..s
                    }),
                    (_, None) => Err(Error::msg(INVALID_DURATION)),
                    (item, Some(v)) => Ok(ParseStep {
                        current_value: None,
                        current_duration: s.current_duration
                            + match item.to_ascii_lowercase() {
                                's' => StdDuration::from_secs(1),
                                'm' => StdDuration::from_secs(60),
                                'h' => StdDuration::from_secs(60 * 60),
                                'd' => StdDuration::from_secs(60 * 60 * 24),
                                'w' => StdDuration::from_secs(60 * 60 * 24 * 7),
                                _ => return Err(Error::msg(INVALID_DURATION)),
                            } * (v as u32),
                    }),
                },
            )
            .and_then(|v| match v.current_value {
                // All values should be consumed
                None => Ok(v),
                _ => Err(Error::msg(INVALID_DURATION)),
            })
            .map(|v| v.current_duration)
    }

    // Our new-orphan-type of duration.
    #[derive(Copy, Clone, Debug)]
    pub struct Duration(pub StdDuration);

    impl std::str::FromStr for Duration {
        type Err = Error;
        fn from_str(s: &str) -> Result<Self, Self::Err> {
            parse_duration_string(s).map(Duration)
        }
    }

    impl From<Duration> for StdDuration {
        fn from(d: Duration) -> Self {
            d.0
        }
    }

    impl Duration {
        fn num_weeks(&self) -> u64 {
            self.0.as_secs() / (60 * 60 * 24 * 7)
        }
        fn num_days(&self) -> u64 {
            self.0.as_secs() / (60 * 60 * 24)
        }
        fn num_hours(&self) -> u64 {
            self.0.as_secs() / (60 * 60)
        }
        fn num_minutes(&self) -> u64 {
            self.0.as_secs() / 60
        }
        fn num_seconds(&self) -> u64 {
            self.0.as_secs()
        }
    }

    impl Duration {
        /// Create a duration from the number of seconds.
        pub fn from_secs(secs: u64) -> Duration {
            Duration(StdDuration::from_secs(secs))
        }
    }

    impl fmt::Display for Duration {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            let d = self;
            // weeks
            let weeks = d.num_weeks();
            let days = d.num_days() - d.num_weeks() * 7;
            let hours = d.num_hours() - d.num_days() * 24;
            let minutes = d.num_minutes() - d.num_hours() * 60;
            let seconds = d.num_seconds() - d.num_minutes() * 60;
            let formats = [
                (weeks, "week"),
                (days, "day"),
                (hours, "hour"),
                (minutes, "minute"),
                (seconds, "second"),
            ];
            let count = f.precision().unwrap_or(formats.len());
            let mut first = true;
            for (val, counter) in formats.iter().skip_while(|(a, _)| *a == 0).take(count) {
                if *val > 0 {
                    write!(
                        f,
                        "{}{} {}{}",
                        (if first { "" } else { " " }),
                        val,
                        counter,
                        (if *val == 1 { "" } else { "s" })
                    )?;
                    first = false;
                }
            }
            Ok(())
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::time::Duration as StdDuration;
        #[test]
        fn test_parse_success() {
            let tests = [
                (
                    "2D2h1m",
                    StdDuration::from_secs(2 * 60 * 60 * 24 + 2 * 60 * 60 + 60),
                ),
                (
                    "1W2D3h4m5s",
                    StdDuration::from_secs(
                        7 * 24 * 60 * 60 + // 1W
                        2 * 24 * 60 * 60 + // 2D
                        3 * 60 * 60 + // 3h
                        4 * 60  + // 4m
                        5, // 5s
                    ),
                ),
                (
                    "1W2D3h4m5s6W",
                    StdDuration::from_secs(
                        7 * 24 * 60 * 60 + // 1W
                        2 * 24 * 60 * 60 + // 2D
                        3 * 60 * 60  + // 3h
                        4 * 60  + // 4m
                        5 + // 5s
                            6 * 7 * 24 * 60 * 60,
                    ), // 6W
                ),
            ];
            for (input, output) in &tests {
                assert_eq!(parse_duration_string(input).unwrap(), *output);
            }
        }

        #[test]
        fn test_parse_fail() {
            let tests = ["", "-1W", "1"];
            for input in &tests {
                assert!(
                    parse_duration_string(input).is_err(),
                    "parsing {} succeeded",
                    input
                );
            }
        }
    }
}

mod username_arg {
    use serenity::model::id::UserId;
    use std::str::FromStr;
    /// An argument that can be either a tagged user, or a raw string.
    pub enum UsernameArg {
        Tagged(UserId),
        Raw(String),
    }

    impl FromStr for UsernameArg {
        type Err = String;
        fn from_str(s: &str) -> Result<Self, Self::Err> {
            match s.parse::<UserId>() {
                Ok(v) => Ok(UsernameArg::Tagged(v)),
                Err(_) if !s.is_empty() => Ok(UsernameArg::Raw(s.to_owned())),
                Err(_) => Err("username arg cannot be empty".to_owned()),
            }
        }
    }

    impl UsernameArg {
        /// Mention yourself.
        pub fn mention<T: Into<UserId>>(v: T) -> Self {
            Self::Tagged(v.into())
        }
    }
}
