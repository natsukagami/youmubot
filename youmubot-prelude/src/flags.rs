use serenity::framework::standard::Args;
use std::collections::HashSet as Set;

/// Handle flags parsing.
pub struct Flags(Set<String>);

struct Flag(pub String);

impl std::str::FromStr for Flag {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.starts_with("--") {
            Ok(Flag(s.trim_start_matches("--").to_owned()))
        } else {
            Err(())
        }
    }
}

impl Flags {
    /// Parses the set of flags from a given `Args` structure.
    pub fn collect_from(args: &mut Args) -> Flags {
        let mut set = Set::new();
        loop {
            if let Ok(Flag(s)) = args.find() {
                set.insert(s);
            } else {
                break Flags(set);
            }
        }
    }

    /// Checks whether `flag` exists in the flags set.
    pub fn contains(&self, flag: impl AsRef<str>) -> bool {
        self.0.contains(flag.as_ref())
    }
}
