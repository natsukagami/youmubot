use announcer::AnnouncerChannels;
/// Module `prelude` provides a sane set of default imports that can be used inside
/// a Youmubot source file.
pub use serenity::prelude::*;
use std::sync::Arc;

pub mod announcer;
pub mod args;
pub mod flags;
pub mod hook;
pub mod member_cache;
pub mod pagination;
pub mod ratelimit;
pub mod setup;

pub use announcer::{Announcer, AnnouncerHandler};
pub use args::{Duration, UsernameArg};
pub use flags::Flags;
pub use hook::Hook;
pub use member_cache::MemberCache;
pub use pagination::{paginate, paginate_fn, paginate_reply, paginate_reply_fn, Paginate};

/// Re-exporting async_trait helps with implementing Announcer.
pub use async_trait::async_trait;

/// Re-export the anyhow errors
pub use anyhow::{anyhow as error, bail, Error, Result};
pub use debugging_ok::OkPrint;

/// Re-export useful future and stream utils
pub use futures_util::{future, stream, FutureExt, StreamExt, TryFutureExt, TryStreamExt};

/// Re-export the spawn function
pub use tokio::spawn as spawn_future;

/// The global app data.
pub type AppData = Arc<RwLock<TypeMap>>;

/// The HTTP client.
pub struct HTTPClient;

/// The global context type for app commands
pub type CmdContext<'a, Env> = poise::Context<'a, Env, anyhow::Error>;

/// The created base environment.
#[derive(Debug, Clone)]
pub struct Env {
    // clients
    pub http: reqwest::Client,
    pub sql: youmubot_db_sql::Pool,
    pub members: Arc<MemberCache>,
    // databases
    // pub(crate) announcer_channels: announcer::AnnouncerChannels,
}

impl TypeMapKey for HTTPClient {
    type Value = reqwest::Client;
}

/// The SQL client.
pub struct SQLClient;

impl TypeMapKey for SQLClient {
    type Value = youmubot_db_sql::Pool;
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

mod debugging_ok {
    pub trait OkPrint {
        type Output;
        fn pls_ok(self) -> Option<Self::Output>;
    }

    impl<T, E: std::fmt::Debug> OkPrint for Result<T, E> {
        type Output = T;

        fn pls_ok(self) -> Option<Self::Output> {
            match self {
                Ok(v) => Some(v),
                Err(e) => {
                    eprintln!("Error: {:?}", e);
                    None
                }
            }
        }
    }
}
