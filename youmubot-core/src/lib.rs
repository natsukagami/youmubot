use serenity::{
    framework::standard::{
        help_commands, macros::help, Args, CommandGroup, CommandResult, HelpOptions,
    },
    model::{channel::Message, id::UserId},
};
use std::collections::HashSet;
use youmubot_prelude::*;

pub mod admin;
pub mod community;
mod db;
pub mod fun;

pub use admin::ADMIN_GROUP;
pub use community::COMMUNITY_GROUP;
pub use fun::FUN_GROUP;

/// Sets up all databases in the client.
pub fn setup(
    path: &std::path::Path,
    client: &serenity::client::Client,
    data: &mut youmubot_prelude::ShareMap,
) -> serenity::framework::standard::CommandResult {
    db::SoftBans::insert_into(&mut *data, &path.join("soft_bans.yaml"))?;
    db::Roles::insert_into(&mut *data, &path.join("roles.yaml"))?;

    // Create handler threads
    std::thread::spawn(admin::watch_soft_bans(client));

    Ok(())
}

// A help command
#[help]
pub fn help(
    context: &mut Context,
    msg: &Message,
    args: Args,
    help_options: &'static HelpOptions,
    groups: &[&'static CommandGroup],
    owners: HashSet<UserId>,
) -> CommandResult {
    help_commands::with_embeds(context, msg, args, help_options, groups, owners)
}
