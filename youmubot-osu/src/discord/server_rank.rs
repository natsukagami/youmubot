use super::{db::OsuSavedUsers, ModeArg};
use crate::models::Mode;
use serenity::{
    framework::standard::{macros::command, Args, CommandResult},
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
pub async fn server_rank(ctx: &Context, m: &Message, mut args: Args) -> CommandResult {
    let data = ctx.data.read().await;
    let mode = args.single::<ModeArg>().map(|v| v.0).unwrap_or(Mode::Std);
    let guild = m.guild_id.expect("Guild-only command");
    let users = OsuSavedUsers::open(&*data).borrow()?.clone();
    let users = users
        .into_iter()
        .map(|(user_id, osu_user)| async move {
            guild.member(&ctx, user_id).await.ok().and_then(|member| {
                osu_user
                    .pp
                    .get(mode as usize)
                    .cloned()
                    .and_then(|pp| pp)
                    .map(|pp| (pp, member.distinct(), osu_user.last_update.clone()))
            })
        })
        .collect::<stream::FuturesUnordered<_>>()
        .filter_map(|v| future::ready(v))
        .collect::<Vec<_>>()
        .await;
    let last_update = users.iter().map(|(_, _, a)| a).min().cloned();
    let mut users = users
        .into_iter()
        .map(|(a, b, _)| (a, b))
        .collect::<Vec<_>>();
    users.sort_by(|(a, _), (b, _)| (*b).partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));

    if users.is_empty() {
        m.reply(&ctx, "No saved users in the current server...")
            .await?;
        return Ok(());
    }

    let users = std::sync::Arc::new(users);
    let last_update = last_update.unwrap();
    paginate(
        move |page: u8, ctx: &Context, m: &mut Message| {
            let users = users.clone();
            Box::pin(async move {
                let start = (page as usize) * ITEMS_PER_PAGE;
                let end = (start + ITEMS_PER_PAGE).min(users.len());
                if start >= end {
                    return Ok(false);
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
                m.edit(ctx, |f| f.content(content.to_string())).await?;
                Ok(true)
            })
        },
        ctx,
        m.channel_id,
        std::time::Duration::from_secs(60),
    )
    .await?;

    Ok(())
}
