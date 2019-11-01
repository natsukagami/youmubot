use serenity::prelude::*;
use serenity::{
    framework::standard::{
        macros::{command, group},
        Args, CommandResult,
    },
    model::channel::Message,
};
use std::{thread::sleep, time::Duration};

group!({
    name: "admin",
    options: {
        only_in: "guilds",
        prefixes: ["admin", "a"],
        description: "Administrative commands for the server.",
    },
    commands: [clean, ban],
});

#[command]
#[aliases("cleanall")]
#[required_permissions(MANAGE_MESSAGES)]
#[description = "Clean at most X latest messages from the current channel. Defaults to 10."]
#[usage = "clean 50"]
fn clean(ctx: &mut Context, msg: &Message, mut args: Args) -> CommandResult {
    let limit = args.single().unwrap_or(10);
    let messages = msg
        .channel_id
        .messages(&ctx.http, |b| b.before(msg.id).limit(limit))?;
    msg.channel_id.delete_messages(&ctx.http, messages.iter())?;
    msg.react(&ctx, "🌋")?;

    sleep(Duration::from_secs(2));
    msg.delete(&ctx)?;

    Ok(())
}

#[command]
#[required_permissions(ADMINISTRATOR)]
#[description = "Ban an user with a certain reason."]
#[usage = "ban user#1234 spam"]
fn ban(ctx: &mut Context, msg: &Message, mut args: Args) -> CommandResult {}
