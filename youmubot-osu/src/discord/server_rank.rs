use super::{db::OsuSavedUsers, ModeArg};
use crate::models::Mode;
use serenity::{
    builder::EditMessage,
    framework::standard::{macros::command, Args, CommandError as Error, CommandResult},
    model::channel::Message,
    utils::MessageBuilder,
};
use std::collections::HashMap;
use youmubot_prelude::*;

#[command("ranks")]
#[description = "See the server's ranks"]
#[usage = "[mode (Std, Taiko, Catch, Mania) = Std]"]
#[max_args(1)]
#[only_in(guilds)]
pub fn server_rank(ctx: &mut Context, m: &Message, mut args: Args) -> CommandResult {
    let mode = args.single::<ModeArg>().map(|v| v.0).unwrap_or(Mode::Std);
    let guild = m.guild_id.expect("Guild-only command");
    let mut users = OsuSavedUsers::open(&*ctx.data.read())
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
                    .map(|pp| (pp, member.distinct()))
            })
        })
        .collect::<Vec<_>>();
    users.sort_by(|(a, _), (b, _)| (*b).partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));

    if users.is_empty() {
        m.reply(&ctx, "No saved users in the current server...")?;
        return Ok(());
    }
    ctx.data.get_cloned::<ReactionWatcher>().paginate_fn(
        ctx.clone(),
        m.channel_id,
        move |page: u8, e: &mut EditMessage| {
            let start = (page as usize) * 5;
            if start >= users.len() {
                return (e, Err(Error("No more items".to_owned())));
            }
            let users = users.iter().skip(start).take(5);
            let mut content = MessageBuilder::new();
            content
                .push_line("```")
                .push_line("Rank | pp      | Username")
                .push_line("-------------------------");
            for (id, (pp, member)) in users.enumerate() {
                content
                    .push(format!(
                        "{:>4} | {:>7.2} | ",
                        format!("#{}", id + start),
                        pp
                    ))
                    .push_line_safe(member);
            }
            content.push("```");
            (e.content(content.build()), Ok(()))
        },
        std::time::Duration::from_secs(60),
    )?;

    Ok(())
}
