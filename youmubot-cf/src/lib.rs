use serenity::{
    framework::standard::{
        macros::{command, group},
        Args, CommandResult,
    },
    model::channel::Message,
};
use youmubot_prelude::*;

mod db;
mod embed;
mod hook;

// /// Live-commentating a Codeforces round.
// pub mod live;

use db::CfSavedUsers;

pub use hook::codeforces_info_hook;

/// Sets up the CF databases.
pub fn setup(path: &std::path::Path, data: &mut ShareMap) {
    CfSavedUsers::insert_into(data, path.join("cf_saved_users.yaml"))
        .expect("Must be able to set up DB")
}

#[group]
#[prefix = "cf"]
#[description = "Codeforces-related commands"]
#[commands(profile, save)]
#[default_command(profile)]
pub struct Codeforces;

#[command]
#[aliases("p", "show", "u", "user", "get")]
#[description = "Get an user's profile"]
#[usage = "[handle or tag = yourself]"]
#[example = "natsukagami"]
#[max_args(1)]
pub fn profile(ctx: &mut Context, m: &Message, mut args: Args) -> CommandResult {
    let handle = args
        .single::<UsernameArg>()
        .unwrap_or(UsernameArg::mention(m.author.id));
    let http = ctx.data.get_cloned::<HTTPClient>();

    let handle = match handle {
        UsernameArg::Raw(s) => s,
        UsernameArg::Tagged(u) => {
            let db = CfSavedUsers::open(&*ctx.data.read());
            let db = db.borrow()?;
            match db.get(&u) {
                Some(v) => v.handle.clone(),
                None => {
                    m.reply(&ctx, "no saved account found.")?;
                    return Ok(());
                }
            }
        }
    };

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

#[command]
#[description = "Link your Codeforces account to the Discord account, to enjoy Youmu's tracking capabilities."]
#[usage = "[handle]"]
#[num_args(1)]
pub fn save(ctx: &mut Context, m: &Message, mut args: Args) -> CommandResult {
    let handle = args.single::<String>()?;
    let http = ctx.data.get_cloned::<HTTPClient>();

    let account = codeforces::User::info(&http, &[&handle[..]])?
        .into_iter()
        .next();

    match account {
        None => {
            m.reply(&ctx, "cannot find an account with such handle")?;
        }
        Some(acc) => {
            let db = CfSavedUsers::open(&*ctx.data.read());
            let mut db = db.borrow_mut()?;
            m.reply(
                &ctx,
                format!("account `{}` has been linked to your account.", &acc.handle),
            )?;
            db.insert(m.author.id, acc.into());
        }
    }

    Ok(())
}
