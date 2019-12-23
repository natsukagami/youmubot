use serenity::prelude::TypeMapKey;

pub(crate) struct HTTP;

impl TypeMapKey for HTTP {
    type Value = reqwest::Client;
}
