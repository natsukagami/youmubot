use std::sync::Arc;
use youmubot_osu::Client as OsuHttpClient;

pub use serenity::prelude::*;

/// The global app data.
pub type AppData = Arc<RwLock<ShareMap>>;

/// The HTTP client.
pub(crate) struct HTTPClient;

impl TypeMapKey for HTTPClient {
    type Value = reqwest::blocking::Client;
}

/// The osu! client.
pub(crate) struct OsuClient;

impl TypeMapKey for OsuClient {
    type Value = OsuHttpClient;
}

/// The TypeMap trait that allows TypeMaps to quickly get a clonable item.
pub trait GetCloned {
    /// Gets an item from the store, cloned.
    fn get_cloned<T>(&self) -> T::Value
    where
        T: TypeMapKey,
        T::Value: Clone + Send + Sync;
}

impl GetCloned for ShareMap {
    fn get_cloned<T>(&self) -> T::Value
    where
        T: TypeMapKey,
        T::Value: Clone + Send + Sync,
    {
        self.get::<T>().cloned().expect("Should be there")
    }
}

impl GetCloned for AppData {
    fn get_cloned<T>(&self) -> T::Value
    where
        T: TypeMapKey,
        T::Value: Clone + Send + Sync,
    {
        self.read().get::<T>().cloned().expect("Should be there")
    }
}
