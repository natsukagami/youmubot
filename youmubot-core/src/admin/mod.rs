use futures_util::{stream, TryStreamExt};
use serenity::{
    framework::standard::{
        macros::{command, group},
        Args, CommandResult,
    },
    model::{
        channel::{Channel, Message},
        id::UserId,
    },
};
use soft_ban::{SOFT_BAN_COMMAND, SOFT_BAN_INIT_COMMAND};
use youmubot_prelude::*;

mod soft_ban;
pub use soft_ban::watch_soft_bans;

#[group]
#[description = "Administrative commands for the server."]
#[commands(clean, ban, kick, soft_ban, soft_ban_init)]
struct Admin;

#[command]
#[aliases("cleanall")]
#[required_permissions(MANAGE_MESSAGES)]
#[description = "Clean at most X latest messages from the current channel (only clean Youmu's messages in DMs). Defaults to 10."]
#[usage = "clean 50"]
#[min_args(0)]
#[max_args(1)]
async fn clean(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let limit = args.single().unwrap_or(10);
    let messages = msg
        .channel_id
        .messages(&ctx.http, |b| b.before(msg.id).limit(limit))
        .await?;
    let channel = msg.channel_id.to_channel(&ctx).await?;
    match &channel {
        Channel::Private(_) => {
            let self_id = ctx.http.get_current_user().await?.id;
            messages
                .into_iter()
                .filter(|v| v.author.id == self_id)
                .map(|m| async move { m.delete(&ctx).await })
                .collect::<stream::FuturesUnordered<_>>()
                .try_collect::<()>()
                .await?;
        }
        _ => {
            msg.channel_id
                .delete_messages(&ctx.http, messages.into_iter())
                .await?;
        }
    };
    msg.react(&ctx, '🌋').await?;
    if let Channel::Guild(_) = &channel {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        msg.delete(&ctx).await.ok();
    }

    Ok(())
}

#[command]
#[required_permissions(ADMINISTRATOR)]
#[description = "Ban an user with a certain reason."]
#[usage = "tag user/[reason = none]/[days of messages to delete = 0]"]
#[min_args(1)]
#[max_args(2)]
#[only_in("guilds")]
async fn ban(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let user = args.single::<UserId>()?.to_user(&ctx).await?;
    let reason = args.single::<String>().map(|v| format!("`{}`", v)).ok();
    let dmds = args.single::<u8>().unwrap_or(0);

    match reason {
        Some(reason) => {
            msg.reply(
                &ctx,
                format!("🔨 Banning user {} for reason `{}`.", user.tag(), reason),
            )
            .await?;
            msg.guild_id
                .ok_or_else(|| Error::msg("Can't get guild from message?"))? // we had a contract
                .ban_with_reason(&ctx.http, user, dmds, &reason)
                .await?;
        }
        None => {
            msg.reply(&ctx, format!("🔨 Banning user {}.", user.tag()))
                .await?;
            msg.guild_id
                .ok_or_else(|| Error::msg("Can't get guild from message?"))? // we had a contract
                .ban(&ctx.http, user, dmds)
                .await?;
        }
    }

    Ok(())
}

#[command]
#[required_permissions(ADMINISTRATOR)]
#[description = "Kick an user."]
#[usage = "@user#1234"]
#[num_args(1)]
#[only_in("guilds")]
async fn kick(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let user = args.single::<UserId>()?.to_user(&ctx).await?;

    msg.reply(&ctx, format!("🔫 Kicking user {}.", user.tag()))
        .await?;

    msg.guild_id
        .ok_or("Can't get guild from message?")? // we had a contract
        .kick(&ctx.http, user)
        .await?;

    Ok(())
}
