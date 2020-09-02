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
