use std::collections::HashSet;

use serenity::{
    framework::standard::{
        help_commands, macros::help, Args, CommandGroup, CommandResult, HelpOptions,
    },
    model::{channel::Message, id::UserId},
};

pub use admin::ADMIN_GROUP;
pub use community::COMMUNITY_GROUP;
pub use fun::FUN_GROUP;
use youmubot_prelude::*;

pub mod admin;
pub mod community;
mod db;
pub mod fun;

#[derive(Debug, Clone)]
pub struct CoreEnv {
    pub(crate) prelude: Env,
    pub(crate) ignore: admin::ignore::IgnoredUsers,
}

impl CoreEnv {
    async fn new(prelude: Env) -> Result<Self> {
        let ignore = admin::ignore::IgnoredUsers::from_db(&prelude).await?;
        Ok(Self { prelude, ignore })
    }
}

/// Gets an [CoreEnv] from the current environment.
pub trait HasCoreEnv: Send + Sync {
    fn core_env(&self) -> &CoreEnv;
}

impl<T: AsRef<CoreEnv> + Send + Sync> HasCoreEnv for T {
    fn core_env(&self) -> &CoreEnv {
        self.as_ref()
    }
}

/// Sets up all databases in the client.
pub async fn setup(path: &std::path::Path, data: &mut TypeMap, prelude: Env) -> Result<CoreEnv> {
    db::load_role_list(
        &mut *data,
        &path.join("roles_v2.yaml"),
        &path.join("roles.yaml"),
    )?;

    // Start reaction handlers
    data.insert::<community::ReactionWatchers>(community::ReactionWatchers::new(&*data)?);

    CoreEnv::new(prelude).await
}

// A help command
#[help]
pub async fn help(
    context: &Context,
    msg: &Message,
    args: Args,
    help_options: &'static HelpOptions,
    groups: &[&'static CommandGroup],
    owners: HashSet<UserId>,
) -> CommandResult {
    help_commands::with_embeds(context, msg, args, help_options, groups, owners).await?;
    Ok(())
}
