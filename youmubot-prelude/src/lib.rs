pub use serenity::prelude::*;
use std::sync::Arc;

pub mod announcer;
pub mod args;
pub mod pagination;
pub mod reaction_watch;
pub mod setup;

pub use announcer::{Announcer, AnnouncerHandler};
pub use args::{Duration, UsernameArg};
pub use pagination::Pagination;
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

pub mod prelude_commands {
    use crate::announcer::ANNOUNCERCOMMANDS_GROUP;
    use serenity::{
        framework::standard::{
            macros::{command, group},
            CommandResult,
        },
        model::channel::Message,
        prelude::Context,
    };

    #[group("Prelude")]
    #[description = "All the commands that makes the base of Youmu"]
    #[commands(ping)]
    #[sub_groups(AnnouncerCommands)]
    pub struct Prelude;

    #[command]
    #[description = "pong!"]
    fn ping(ctx: &mut Context, m: &Message) -> CommandResult {
        m.reply(&ctx, "Pong!")?;
        Ok(())
    }
}