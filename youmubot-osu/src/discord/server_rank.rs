use super::{
    cache::get_beatmap,
    db::{OsuSavedUsers, OsuUserBests},
    ModeArg,
};
use crate::models::{Mode, Score};
use serenity::{
    builder::EditMessage,
    framework::standard::{macros::command, Args, CommandError as Error, CommandResult},
    model::channel::Message,
    utils::MessageBuilder,
};
use youmubot_prelude::*;

#[command("ranks")]
#[description = "See the server's ranks"]
#[usage = "[mode (Std, Taiko, Catch, Mania) = Std]"]
#[max_args(1)]
#[only_in(guilds)]
pub fn server_rank(ctx: &mut Context, m: &Message, mut args: Args) -> CommandResult {
    let mode = args.single::<ModeArg>().map(|v| v.0).unwrap_or(Mode::Std);
    let guild = m.guild_id.expect("Guild-only command");
    let users = OsuSavedUsers::open(&*ctx.data.read())
        .borrow()
        .expect("DB initialized")
        .iter()
        .filter_map(|(user_id, osu_user)| {
            guild.member(&ctx, user_id).ok().and_then(|member| {
                osu_user
                    .pp
                    .get(mode as usize)
                    .cloned()
                    .and_then(|pp| pp)
                    .map(|pp| (pp, member.distinct(), osu_user.last_update.clone()))
            })
        })
        .collect::<Vec<_>>();
    let last_update = users.iter().map(|(_, _, a)| a).min().cloned();
    let mut users = users
        .into_iter()
        .map(|(a, b, _)| (a, b))
        .collect::<Vec<_>>();
    users.sort_by(|(a, _), (b, _)| (*b).partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));

    if users.is_empty() {
        m.reply(&ctx, "No saved users in the current server...")?;
        return Ok(());
    }
    let last_update = last_update.unwrap();
    const ITEMS_PER_PAGE: usize = 10;
    ctx.data.get_cloned::<ReactionWatcher>().paginate_fn(
        ctx.clone(),
        m.channel_id,
        move |page: u8, e: &mut EditMessage| {
            let start = (page as usize) * ITEMS_PER_PAGE;
            let end = (start + ITEMS_PER_PAGE).min(users.len());
            if start >= end {
                return (e, Err(Error("No more items".to_owned())));
            }
            let total_len = users.len();
            let users = &users[start..end];
            let username_len = users.iter().map(|(_, u)| u.len()).max().unwrap().max(8);
            let mut content = MessageBuilder::new();
            content
                .push_line("```")
                .push_line("Rank | pp      | Username")
                .push_line(format!("-----------------{:-<uw$}", "", uw = username_len));
            for (id, (pp, member)) in users.iter().enumerate() {
                content
                    .push(format!(
                        "{:>4} | {:>7.2} | ",
                        format!("#{}", 1 + id + start),
                        pp
                    ))
                    .push_line_safe(member);
            }
            content.push_line("```").push_line(format!(
                "Page **{}**/**{}**. Last updated: `{}`",
                page + 1,
                (total_len + ITEMS_PER_PAGE - 1) / ITEMS_PER_PAGE,
                last_update.to_rfc2822()
            ));
            (e.content(content.build()), Ok(()))
        },
        std::time::Duration::from_secs(60),
    )?;

    Ok(())
}

#[command("leaderboard")]
#[aliases("lb", "bmranks", "br", "cc")]
#[description = "See the server's ranks on the last seen beatmap"]
#[max_args(0)]
#[only_in(guilds)]
pub fn leaderboard(ctx: &mut Context, m: &Message, mut _args: Args) -> CommandResult {
    let bm = match get_beatmap(&*ctx.data.read(), m.channel_id)? {
        Some(bm) => bm,
        None => {
            m.reply(&ctx, "No beatmap queried on this channel.")?;
            return Ok(());
        }
    };

    let guild = m.guild_id.expect("Guild-only command");
    let scores = {
        let users = OsuUserBests::open(&*ctx.data.read());
        let users = users.borrow()?;
        let users = match users.get(&(bm.0.beatmap_id, bm.1)) {
            None => {
                m.reply(
                &ctx,
                "No scores have been recorded for this beatmap. Run `osu check` to scan for yours!",
            )?;
                return Ok(());
            }
            Some(v) if v.is_empty() => {
                m.reply(
                &ctx,
                "No scores have been recorded for this beatmap. Run `osu check` to scan for yours!",
            )?;
                return Ok(());
            }
            Some(v) => v,
        };

        let mut scores: Vec<(f64, String, Score)> = users
            .iter()
            .filter_map(|(user_id, scores)| {
                guild
                    .member(&ctx, user_id)
                    .ok()
                    .and_then(|m| Some((m.distinct(), scores)))
            })
            .flat_map(|(user, scores)| scores.into_iter().map(move |v| (user.clone(), v.clone())))
            .filter_map(|(user, score)| score.pp.map(|v| (v, user, score)))
            .collect::<Vec<_>>();
        scores
            .sort_by(|(a, _, _), (b, _, _)| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
        scores
    };

    if scores.is_empty() {
        m.reply(
            &ctx,
            "No scores have been recorded for this beatmap. Run `osu check` to scan for yours!",
        )?;
        return Ok(());
    }
    ctx.data.get_cloned::<ReactionWatcher>().paginate_fn(
        ctx.clone(),
        m.channel_id,
        move |page: u8, e: &mut EditMessage| {
            const ITEMS_PER_PAGE: usize = 5;
            let start = (page as usize) * ITEMS_PER_PAGE;
            let end = (start + ITEMS_PER_PAGE).min(scores.len());
            if start >= end {
                return (e, Err(Error("No more items".to_owned())));
            }
            let total_len = scores.len();
            let scores = &scores[start..end];
            // username width
            let uw = scores
                .iter()
                .map(|(_, u, _)| u.len())
                .max()
                .unwrap_or(8)
                .max(8);
            let accuracies = scores
                .iter()
                .map(|(_, _, v)| format!("{:.2}%", v.accuracy(bm.1)))
                .collect::<Vec<_>>();
            let aw = accuracies.iter().map(|v| v.len()).max().unwrap().max(3);
            let misses = scores
                .iter()
                .map(|(_, _, v)| format!("{}", v.count_miss))
                .collect::<Vec<_>>();
            let mw = misses.iter().map(|v| v.len()).max().unwrap().max(4);
            let ranks = scores
                .iter()
                .map(|(_, _, v)| v.rank.to_string())
                .collect::<Vec<_>>();
            let rw = ranks.iter().map(|v| v.len()).max().unwrap().max(4);
            let pp = scores
                .iter()
                .map(|(pp, _, _)| format!("{:.2}", pp))
                .collect::<Vec<_>>();
            let pw = pp.iter().map(|v| v.len()).max().unwrap_or(2);
            /*mods width*/
            let mdw = scores
                .iter()
                .map(|(_, _, v)| v.mods.to_string().len())
                .max()
                .unwrap()
                .max(4);
            let mut content = MessageBuilder::new();
            content
                .push_line("```")
                .push_line(format!(
                    "rank | {:>pw$} | {:mdw$} | {:rw$} | {:>aw$} | {:mw$} | {:uw$}",
                    "pp",
                    "mods",
                    "rank",
                    "acc",
                    "miss",
                    "user",
                    pw = pw,
                    mdw = mdw,
                    rw = rw,
                    aw = aw,
                    mw = mw,
                    uw = uw,
                ))
                .push_line(format!(
                    "-------{:-<pw$}---{:-<mdw$}---{:-<rw$}---{:-<aw$}---{:-<mw$}---{:-<uw$}",
                    "",
                    "",
                    "",
                    "",
                    "",
                    "",
                    pw = pw,
                    mdw = mdw,
                    rw = rw,
                    aw = aw,
                    mw = mw,
                    uw = uw,
                ));
            for (id, (_, member, p)) in scores.iter().enumerate() {
                content.push_line_safe(format!(
                    "{:>4} | {:>pw$} | {:>mdw$} | {:>rw$} | {:>aw$} | {:>mw$} | {:uw$}",
                    format!("#{}", 1 + id + start),
                    pp[id],
                    p.mods.to_string(),
                    ranks[id],
                    accuracies[id],
                    misses[id],
                    member,
                    pw = pw,
                    mdw = mdw,
                    rw = rw,
                    aw = aw,
                    mw = mw,
                    uw = uw,
                ));
            }
            content.push_line("```").push_line(format!(
                "Page **{}**/**{}**. Not seeing your scores? Run `osu check` to update.",
                page + 1,
                (total_len + ITEMS_PER_PAGE - 1) / ITEMS_PER_PAGE,
            ));
            (e.content(content.build()), Ok(()))
        },
        std::time::Duration::from_secs(60),
    )?;

    Ok(())
}
