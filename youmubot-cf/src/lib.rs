use serenity::{
    framework::standard::{
        macros::{command, group},
        Args, CommandError as Error, CommandResult,
    },
    model::channel::Message,
    utils::MessageBuilder,
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
#[commands(profile, save, ranks)]
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

#[command]
#[description = "See the leaderboard of all people in the server."]
#[only_in(guilds)]
#[num_args(0)]
pub fn ranks(ctx: &mut Context, m: &Message) -> CommandResult {
    let everyone = {
        let db = CfSavedUsers::open(&*ctx.data.read());
        let db = db.borrow()?;
        db.iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect::<Vec<_>>()
    };
    let guild = m.guild_id.expect("Guild-only command");
    let mut ranks = everyone
        .into_iter()
        .filter_map(|(id, cf_user)| guild.member(&ctx, id).ok().map(|mem| (mem, cf_user)))
        .collect::<Vec<_>>();
    ranks.sort_by(|(_, a), (_, b)| b.rating.unwrap_or(-1).cmp(&a.rating.unwrap_or(-1)));

    if ranks.is_empty() {
        m.reply(&ctx, "No saved users in this server.")?;
        return Ok(());
    }

    const ITEMS_PER_PAGE: usize = 10;
    let total_pages = (ranks.len() + ITEMS_PER_PAGE - 1) / ITEMS_PER_PAGE;
    let last_updated = ranks.iter().map(|(_, cfu)| cfu.last_update).min().unwrap();

    ctx.data.get_cloned::<ReactionWatcher>().paginate_fn(
        ctx.clone(),
        m.channel_id,
        |page, e| {
            let page = page as usize;
            let start = ITEMS_PER_PAGE * page;
            let end = ranks.len().min(start + ITEMS_PER_PAGE);
            if start >= end {
                return (e, Err(Error::from("No more pages")));
            }
            let ranks = &ranks[start..end];

            let handle_width = ranks.iter().map(|(_, cfu)| cfu.handle.len()).max().unwrap();
            let username_width = ranks
                .iter()
                .map(|(mem, _)| mem.distinct().len())
                .max()
                .unwrap();

            let mut m = MessageBuilder::new();
            m.push_line("```");

            // Table header
            m.push_line(format!(
                "Rank | Rating | {:hw$} | {:uw$}",
                "Handle",
                "Username",
                hw = handle_width,
                uw = username_width
            ));
            m.push_line(format!(
                "----------------{:->hw$}---{:->uw$}",
                "",
                "",
                hw = handle_width,
                uw = username_width
            ));

            for (id, (mem, cfu)) in ranks.iter().enumerate() {
                let id = id + start + 1;
                m.push_line(format!(
                    "{:>4} | {:>6} | {:hw$} | {:uw$}",
                    format!("#{}", id),
                    cfu.rating
                        .map(|v| v.to_string())
                        .unwrap_or("----".to_owned()),
                    cfu.handle,
                    mem.distinct(),
                    hw = handle_width,
                    uw = username_width
                ));
            }

            m.push_line("```");
            m.push(format!(
                "Page **{}/{}**. Last updated **{}**",
                page + 1,
                total_pages,
                last_updated.to_rfc2822()
            ));

            (e.content(m.build()), Ok(()))
        },
        std::time::Duration::from_secs(60),
    )?;

    Ok(())
}
