use rand::{
    distributions::{Distribution, Uniform},
    thread_rng,
};
use serenity::{
    framework::standard::{
        macros::{command, group},
        Args, CommandError as Error, CommandResult,
    },
    model::{
        channel::{Channel, Message},
        id::RoleId,
        user::OnlineStatus,
    },
    utils::MessageBuilder,
};
use youmubot_prelude::*;

mod roles;
mod votes;

use roles::{
    ADD_COMMAND, LIST_COMMAND, REMOVE_COMMAND, RMROLEMESSAGE_COMMAND, ROLEMESSAGE_COMMAND,
    TOGGLE_COMMAND, UPDATEROLEMESSAGE_COMMAND,
};
use votes::VOTE_COMMAND;

pub use roles::ReactionWatchers;

#[group]
#[description = "Community related commands. Usually comes with some sort of delays, since it involves pinging"]
#[only_in("guilds")]
#[commands(
    choose,
    vote,
    add,
    list,
    remove,
    toggle,
    rolemessage,
    rmrolemessage,
    updaterolemessage
)]
struct Community;

#[command]
#[description = r"👑 Randomly choose an active member and mention them!
Note that only online/idle users in the channel are chosen from."]
#[usage = "[limited roles = everyone online] / [title = the chosen one]"]
#[example = "the strongest in Gensokyo"]
#[bucket = "community"]
#[max_args(2)]
pub async fn choose(ctx: &Context, m: &Message, mut args: Args) -> CommandResult {
    let flags = Flags::collect_from(&mut args);
    let role = args.find::<RoleId>().ok();
    let title = if args.is_empty() {
        "the chosen one".to_owned()
    } else {
        args.single::<String>()?
    };

    let online_only = !flags.contains("everyone");

    let users: Result<Vec<_>, Error> = {
        let guild = m.guild(&ctx).unwrap();
        let presences = &guild.presences;
        let channel = m.channel_id.to_channel(&ctx).await?;
        if let Channel::Guild(channel) = channel {
            Ok(channel
                .members(&ctx)
                .await?
                .into_iter()
                .filter(|v| !v.user.bot) // Filter out bots
                .filter(|v| {
                    if !online_only {
                        return true;
                    }
                    // Filter out only online people
                    presences
                        .get(&v.user.id)
                        .map(|presence| {
                            presence.status == OnlineStatus::Online
                                || presence.status == OnlineStatus::Idle
                        })
                        .unwrap_or(false)
                })
                .map(future::ready)
                .collect::<stream::FuturesUnordered<_>>()
                .filter_map(|member| async move {
                    // Filter by role if provided
                    match role {
                        Some(role) if member.roles.iter().any(|r| role == *r) => Some(member),
                        None => Some(member),
                        _ => None,
                    }
                })
                .collect()
                .await)
        } else {
            unreachable!()
        }
    };
    let users = users?;

    if users.len() < 2 {
        m.reply(
            &ctx,
            "🍰 Have this cake for yourself because no-one is here for the gods to pick.",
        )
        .await?;
        return Ok(());
    }

    let winner = {
        let uniform = Uniform::from(0..users.len());
        let mut rng = thread_rng();
        &users[uniform.sample(&mut rng)]
    };

    m.channel_id
        .send_message(&ctx, |c| {
            c.content(
                MessageBuilder::new()
                    .push("👑 The Gensokyo gods have gathered around and decided, out of ")
                    .push_bold(format!("{}", users.len()))
                    .push(" ")
                    .push(
                        role.map(|r| format!("{}s", r.mention()))
                            .unwrap_or_else(|| "potential prayers".to_owned()),
                    )
                    .push(", ")
                    .push(winner.mention())
                    .push(" will be ")
                    .push_bold_safe(title)
                    .push(". Congrats! 🎉 🎊 🥳")
                    .build(),
            )
            .reference_message(m)
        })
        .await?;

    Ok(())
}
