use rand::{
    distributions::{Distribution, Uniform},
    thread_rng,
};
use serenity::prelude::*;
use serenity::{
    framework::standard::{
        macros::{command, group},
        Args, CommandError as Error, CommandResult,
    },
    model::{
        channel::{Channel, Message},
        user::OnlineStatus,
    },
    utils::MessageBuilder,
};

mod votes;

use votes::VOTE_COMMAND;

#[group]
#[description = "Community related commands. Usually comes with some sort of delays, since it involves pinging"]
#[only_in("guilds")]
#[commands(choose, vote)]
struct Community;

#[command]
#[description = r"👑 Randomly choose an active member and mention them!
Note that only online/idle users in the channel are chosen from."]
#[usage = "[title = the chosen one]"]
#[example = "the strongest in Gensokyo"]
#[bucket = "community"]
#[max_args(1)]
pub fn choose(ctx: &mut Context, m: &Message, mut args: Args) -> CommandResult {
    let title = if args.is_empty() {
        "the chosen one".to_owned()
    } else {
        args.single::<String>()?
    };

    let users: Result<Vec<_>, Error> = {
        let guild = m.guild(&ctx).unwrap();
        let guild = guild.read();
        let presences = &guild.presences;
        let channel = m.channel_id.to_channel(&ctx)?;
        if let Channel::Guild(channel) = channel {
            let channel = channel.read();
            Ok(channel
                .members(&ctx)?
                .into_iter()
                .filter(|v| !v.user.read().bot)
                .map(|v| v.user_id())
                .filter(|v| {
                    presences
                        .get(v)
                        .map(|presence| {
                            presence.status == OnlineStatus::Online
                                || presence.status == OnlineStatus::Idle
                        })
                        .unwrap_or(false)
                })
                .collect())
        } else {
            panic!()
        }
    };
    let users = users?;

    if users.len() < 2 {
        m.reply(
            &ctx,
            "🍰 Have this cake for yourself because no-one is here for the gods to pick.",
        )?;
        return Ok(());
    }

    let winner = {
        let uniform = Uniform::from(0..users.len());
        let mut rng = thread_rng();
        &users[uniform.sample(&mut rng)]
    };

    m.channel_id.send_message(&ctx, |c| {
        c.content(
            MessageBuilder::new()
                .push("👑 The Gensokyo gods have gathered around and decided, out of ")
                .push_bold(format!("{}", users.len()))
                .push(" potential prayers, ")
                .push(winner.mention())
                .push(" will be ")
                .push_bold_safe(title)
                .push(". Congrats! 🎉 🎊 🥳")
                .build(),
        )
    })?;

    Ok(())
}
