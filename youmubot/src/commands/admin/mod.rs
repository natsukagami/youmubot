use serenity::prelude::*;
use serenity::{
    framework::standard::{
        macros::{command, group},
        Args, CommandResult,
    },
    model::{channel::Message, id::UserId},
};
use soft_ban::{SOFT_BAN_COMMAND, SOFT_BAN_INIT_COMMAND};
use std::{thread::sleep, time::Duration};

mod soft_ban;
pub use soft_ban::watch_soft_bans;

group!({
    name: "admin",
    options: {
        only_in: "guilds",
        description: "Administrative commands for the server.",
    },
    commands: [clean, ban, kick, soft_ban, soft_ban_init],
});

#[command]
#[aliases("cleanall")]
#[required_permissions(MANAGE_MESSAGES)]
#[description = "Clean at most X latest messages from the current channel. Defaults to 10."]
#[usage = "clean 50"]
#[min_args(0)]
#[max_args(1)]
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
#[min_args(1)]
#[max_args(2)]
fn ban(ctx: &mut Context, msg: &Message, mut args: Args) -> CommandResult {
    let user = args.single::<UserId>()?.to_user(&ctx)?;
    let reason = args
        .remains()
        .map(|v| format!("`{}`", v))
        .unwrap_or("no provided reason".to_owned());

    msg.reply(
        &ctx,
        format!("🔨 Banning user {} for reason `{}`.", user.tag(), reason),
    )?;

    msg.guild_id
        .ok_or("Can't get guild from message?")? // we had a contract
        .ban(&ctx.http, user, &reason)?;

    Ok(())
}

#[command]
#[required_permissions(ADMINISTRATOR)]
#[description = "Kick an user with a certain reason."]
#[usage = "kick user#1234 spam"]
#[min_args(1)]
#[max_args(2)]
fn kick(ctx: &mut Context, msg: &Message, mut args: Args) -> CommandResult {
    let user = args.single::<UserId>()?.to_user(&ctx)?;
    let reason = args
        .remains()
        .map(|v| format!("`{}`", v))
        .unwrap_or("no provided reason".to_owned());

    msg.reply(
        &ctx,
        format!("🔫 Kicking user {} for {}.", user.tag(), reason),
    )?;

    msg.guild_id
        .ok_or("Can't get guild from message?")? // we had a contract
        .ban(&ctx.http, user, &reason)?;

    Ok(())
}
