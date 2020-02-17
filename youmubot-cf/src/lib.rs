use codeforces::Contest;
use serenity::{
    framework::standard::{
        macros::{command, group},
        Args, CommandError as Error, CommandResult,
    },
    model::channel::Message,
    utils::MessageBuilder,
};
use std::{collections::HashMap, time::Duration};
use youmubot_prelude::*;

mod announcer;
mod db;
mod embed;
mod hook;

/// Live-commentating a Codeforces round.
mod live;

use db::{CfSavedUsers, CfUser};

pub use hook::codeforces_info_hook;

/// Sets up the CF databases.
pub fn setup(path: &std::path::Path, data: &mut ShareMap, announcers: &mut AnnouncerHandler) {
    CfSavedUsers::insert_into(data, path.join("cf_saved_users.yaml"))
        .expect("Must be able to set up DB");
    data.insert::<hook::ContestCache>(hook::ContestCache::default());
    announcers.add("codeforces", announcer::updates);
}

#[group]
#[prefix = "cf"]
#[description = "Codeforces-related commands"]
#[commands(profile, save, ranks, watch, contestranks)]
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
            // Collect rating changes data.
            let rating_changes = acc.rating_changes(&http)?;
            let db = CfSavedUsers::open(&*ctx.data.read());
            let mut db = db.borrow_mut()?;
            m.reply(
                &ctx,
                format!("account `{}` has been linked to your account.", &acc.handle),
            )?;
            db.insert(m.author.id, CfUser::save(acc, rating_changes));
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

#[command]
#[description = "See the server ranks on a certain contest"]
#[usage = "[the contest id]"]
#[num_args(1)]
#[only_in(guilds)]
pub fn contestranks(ctx: &mut Context, m: &Message, mut args: Args) -> CommandResult {
    let contest_id: u64 = args.single()?;
    let guild = m.guild_id.unwrap(); // Guild-only command
    let members = CfSavedUsers::open(&*ctx.data.read()).borrow()?.clone();
    let members = members
        .into_iter()
        .filter_map(|(user_id, cf_user)| {
            guild
                .member(&ctx, user_id)
                .ok()
                .map(|v| (cf_user.handle, v))
        })
        .collect::<HashMap<_, _>>();
    let http = ctx.data.get_cloned::<HTTPClient>();
    let (contest, problems, ranks) = Contest::standings(&http, contest_id, |f| {
        f.handles(members.iter().map(|(k, _)| k.clone()).collect())
    })?;

    // Table me
    let ranks = ranks
        .iter()
        .flat_map(|v| {
            v.party
                .members
                .iter()
                .filter_map(|m| members.get(&m.handle).map(|mem| (mem, m.handle.clone(), v)))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    if ranks.is_empty() {
        m.reply(&ctx, "No one in this server participated in the contest...")?;
        return Ok(());
    }

    const ITEMS_PER_PAGE: usize = 10;
    let total_pages = (ranks.len() + ITEMS_PER_PAGE - 1) / ITEMS_PER_PAGE;

    ctx.data.get_cloned::<ReactionWatcher>().paginate_fn(
        ctx.clone(),
        m.channel_id,
        move |page, e| {
            let page = page as usize;
            let start = page * ITEMS_PER_PAGE;
            let end = ranks.len().min(start + ITEMS_PER_PAGE);
            if start >= end {
                return (e, Err(Error::from("no more pages to show")));
            }
            let ranks = &ranks[start..end];
            let hw = ranks
                .iter()
                .map(|(mem, handle, _)| format!("{} ({})", handle, mem.distinct()).len())
                .max()
                .unwrap_or(0)
                .max(6);
            let hackw = ranks
                .iter()
                .map(|(_, _, row)| {
                    format!(
                        "{}/{}",
                        row.successful_hack_count, row.unsuccessful_hack_count
                    )
                    .len()
                })
                .max()
                .unwrap_or(0)
                .max(5);

            let mut table = MessageBuilder::new();
            let mut header = MessageBuilder::new();
            // Header
            header.push(format!(
                " Rank | {:hw$} | Total | {:hackw$}",
                "Handle",
                "Hacks",
                hw = hw,
                hackw = hackw
            ));
            for p in &problems {
                header.push(format!(" | {:4}", p.index));
            }
            let header = header.build();
            table
                .push_line(&header)
                .push_line(format!("{:-<w$}", "", w = header.len()));

            // Body
            for (mem, handle, row) in ranks {
                table.push(format!(
                    "{:>5} | {:<hw$} | {:>5.0} | {:<hackw$}",
                    row.rank,
                    format!("{} ({})", handle, mem.distinct()),
                    row.points,
                    format!(
                        "{}/{}",
                        row.successful_hack_count, row.unsuccessful_hack_count
                    ),
                    hw = hw,
                    hackw = hackw
                ));
                for p in &row.problem_results {
                    table.push(" | ");
                    if p.points > 0.0 {
                        table.push(format!("{:^4.0}", p.points));
                    } else if let Some(_) = p.best_submission_time_seconds {
                        table.push(format!("{:^4}", "?"));
                    } else if p.rejected_attempt_count > 0 {
                        table.push(format!("{:^4}", format!("-{}", p.rejected_attempt_count)));
                    } else {
                        table.push(format!("{:^4}", ""));
                    }
                }
            }

            let mut m = MessageBuilder::new();
            m.push_bold_safe(&contest.name)
                .push(" ")
                .push_line(contest.url())
                .push_codeblock(table.build(), None)
                .push_line(format!("Page **{}/{}**", page + 1, total_pages));
            (e.content(m.build()), Ok(()))
        },
        Duration::from_secs(60),
    )
}

#[command]
#[description = "Watch a contest and announce any change on the members of the server assigned to the contest."]
#[usage = "[the contest id]"]
#[num_args(1)]
#[required_permissions(MANAGE_CHANNELS)]
#[only_in(guilds)]
pub fn watch(ctx: &mut Context, m: &Message, mut args: Args) -> CommandResult {
    let contest_id: u64 = args.single()?;

    live::watch_contest(ctx, m.guild_id.unwrap(), m.channel_id, contest_id)?;

    Ok(())
}
