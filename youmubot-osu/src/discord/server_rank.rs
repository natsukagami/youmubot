use super::{db::OsuSavedUsers, ModeArg};
use crate::models::Mode;
use serenity::{
    builder::EditMessage,
    framework::standard::{macros::command, Args, CommandError as Error, CommandResult},
    model::channel::Message,
    utils::MessageBuilder,
};
use youmubot_prelude::*;

const ITEMS_PER_PAGE: usize = 10;

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
            let username_len = users.iter().map(|(_, u)| u.len()).max().unwrap_or(8).max(8);
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
