pub use serenity::prelude::*;
use std::sync::Arc;

pub mod announcer;
pub mod args;
pub mod reaction_watch;
pub mod setup;

pub use announcer::Announcer;
pub use args::Duration;
pub use reaction_watch::{ReactionHandler, ReactionWatcher};

/// The global app data.
pub type AppData = Arc<RwLock<ShareMap>>;

/// The HTTP client.
pub struct HTTPClient;

impl TypeMapKey for HTTPClient {
    type Value = reqwest::blocking::Client;
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
