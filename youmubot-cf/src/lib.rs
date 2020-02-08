use serenity::{
    framework::standard::{
        macros::{command, group},
        Args, CommandResult,
    },
    model::{channel::Message, id::UserId},
};
use youmubot_prelude::*;

mod embed;
mod hook;

pub use hook::codeforces_info_hook;

#[group]
#[prefix = "cf"]
#[description = "Codeforces-related commands"]
#[commands(profile)]
#[default_command(profile)]
pub struct Codeforces;

#[command]
#[aliases("p", "show", "u", "user", "get")]
#[description = "Get an user's profile"]
#[usage = "[handle or tag = yourself]"]
#[example = "natsukagami"]
#[num_args(1)]
pub fn profile(ctx: &mut Context, m: &Message, mut args: Args) -> CommandResult {
    let handle = args.single::<String>()?;
    let http = ctx.data.get_cloned::<HTTPClient>();

    let account = codeforces::User::info(&http, &[&handle[..]])?
        .into_iter()
        .next();

    match account {
        Some(v) => m.channel_id.send_message(&ctx, |send| {
            send.content(format!(
                "{}: Here is the user that you requested",
                m.author.mention()
            ))
            .embed(|e| embed::user_embed(&v, e))
        }),
        None => m.reply(&ctx, "User not found"),
    }?;

    Ok(())
}
