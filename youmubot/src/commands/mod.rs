use serenity::prelude::*;
use serenity::{
    framework::standard::{
        help_commands, macros::help, Args, CommandGroup, CommandResult, HelpOptions,
    },
    model::{channel::Message, id::UserId},
};
use std::collections::HashSet;

mod args;

pub mod admin;
pub mod community;
pub mod fun;
pub mod osu;

pub use admin::ADMIN_GROUP;
pub use community::COMMUNITY_GROUP;
pub use fun::FUN_GROUP;

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
