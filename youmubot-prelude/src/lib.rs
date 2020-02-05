pub use serenity::prelude::*;
use std::sync::Arc;

pub mod announcer;
pub mod args;
pub mod setup;

pub use announcer::Announcer;
pub use args::Duration;

/// The global app data.
pub type AppData = Arc<RwLock<ShareMap>>;

/// The HTTP client.
pub struct HTTPClient;

impl TypeMapKey for HTTPClient {
    type Value = reqwest::blocking::Client;
}

/// The osu! client.
// pub(crate) struct OsuClient;

// impl TypeMapKey for OsuClient {
//     type Value = OsuHttpClient;
// }

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
