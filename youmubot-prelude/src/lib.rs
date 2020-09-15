/// Module `prelude` provides a sane set of default imports that can be used inside
/// a Youmubot source file.
pub use serenity::prelude::*;
use std::sync::Arc;

pub mod announcer;
pub mod args;
pub mod hook;
pub mod pagination;
pub mod ratelimit;
pub mod setup;

pub use announcer::{Announcer, AnnouncerHandler};
pub use args::{Duration, UsernameArg};
pub use hook::Hook;
pub use pagination::paginate;

/// Re-exporting async_trait helps with implementing Announcer.
pub use async_trait::async_trait;

/// Re-export the anyhow errors
pub use anyhow::{Error, Result};

/// Re-export useful future and stream utils
pub use futures_util::{future, stream, FutureExt, StreamExt, TryFutureExt, TryStreamExt};

/// Re-export the spawn function
pub use tokio::spawn as spawn_future;

/// The global app data.
pub type AppData = Arc<RwLock<TypeMap>>;

/// The HTTP client.
pub struct HTTPClient;

impl TypeMapKey for HTTPClient {
    type Value = reqwest::Client;
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
    async fn ping(ctx: &Context, m: &Message) -> CommandResult {
        m.reply(&ctx, "Pong!").await?;
        Ok(())
    }
}
