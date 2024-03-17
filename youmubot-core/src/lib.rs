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
use youmubot_prelude::{announcer::CacheAndHttp, *};

pub mod admin;
pub mod community;
mod db;
pub mod fun;

/// Sets up all databases in the client.
pub fn setup(
    path: &std::path::Path,
    data: &mut TypeMap,
) -> serenity::framework::standard::CommandResult {
    db::SoftBans::insert_into(&mut *data, &path.join("soft_bans.yaml"))?;
    db::load_role_list(
        &mut *data,
        &path.join("roles_v2.yaml"),
        &path.join("roles.yaml"),
    )?;

    // Start reaction handlers
    data.insert::<community::ReactionWatchers>(community::ReactionWatchers::new(&*data)?);

    Ok(())
}

pub fn ready_hook(ctx: &Context) -> CommandResult {
    // Create handler threads
    tokio::spawn(admin::watch_soft_bans(
        CacheAndHttp::from_context(ctx),
        ctx.data.clone(),
    ));
    Ok(())
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
