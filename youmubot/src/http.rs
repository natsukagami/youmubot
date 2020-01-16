use serenity::prelude::TypeMapKey;
use youmubot_osu::Client as OsuClient;

pub(crate) struct HTTP;

impl TypeMapKey for HTTP {
    type Value = reqwest::blocking::Client;
}

pub(crate) struct Osu;

impl TypeMapKey for Osu {
    type Value = OsuClient;
}
