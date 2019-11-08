pub use duration::Duration;

mod duration {
    use chrono::Duration as StdDuration;
    use serenity::framework::standard::CommandError as Error;
    // Parse a single duration unit
    fn parse_duration_string(s: &str) -> Result<StdDuration, Error> {
        // We reject the empty case
        if s == "" {
            return Err(Error::from("empty strings are not valid durations"));
        }
        struct ParseStep {
            current_value: Option<u64>,
            current_duration: StdDuration,
        }
        s.chars()
            .try_fold(
                ParseStep {
                    current_value: None,
                    current_duration: StdDuration::zero(),
                },
                |s, item| match (item, s.current_value) {
                    ('0'..='9', v) => Ok(ParseStep {
                        current_value: Some(v.unwrap_or(0) * 10 + ((item as u64) - ('0' as u64))),
                        ..s
                    }),
                    (_, None) => Err(Error::from("Not a valid duration")),
                    (item, Some(v)) => Ok(ParseStep {
                        current_value: None,
                        current_duration: s.current_duration
                            + match item {
                                's' => StdDuration::seconds,
                                'm' => StdDuration::minutes,
                                'h' => StdDuration::hours,
                                'D' => StdDuration::days,
                                'W' => StdDuration::weeks,
                                _ => return Err(Error::from("Not a valid duration")),
                            }(v as i64),
                    }),
                },
            )
            .and_then(|v| match v.current_value {
                // All values should be consumed
                None => Ok(v),
                _ => Err(Error::from("Not a valid duration")),
            })
            .map(|v| v.current_duration)
    }

    // Our new-orphan-type of duration.
    #[derive(Copy, Clone, Debug)]
    pub struct Duration(pub StdDuration);

    impl std::str::FromStr for Duration {
        type Err = Error;
        fn from_str(s: &str) -> Result<Self, Self::Err> {
            parse_duration_string(s).map(|v| Duration(v))
        }
    }

    impl From<Duration> for StdDuration {
        fn from(d: Duration) -> Self {
            d.0
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use chrono::Duration as StdDuration;
        #[test]
        fn test_parse_success() {
            let tests = [
                (
                    "2D2h1m",
                    StdDuration::seconds(2 * 60 * 60 * 24 + 2 * 60 * 60 + 1 * 60),
                ),
                (
                    "1W2D3h4m5s",
                    StdDuration::seconds(
                        1 * 7 * 24 * 60 * 60 + // 1W
                        2 * 24 * 60 * 60 + // 2D
                        3 * 60 * 60 + // 3h
                        4 * 60  + // 4m
                        5, // 5s
                    ),
                ),
                (
                    "1W2D3h4m5s6W",
                    StdDuration::seconds(
                        1 * 7 * 24 * 60 * 60 + // 1W
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
            let tests = ["", "1w", "-1W", "1"];
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
