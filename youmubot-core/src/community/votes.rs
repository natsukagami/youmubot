use serenity::framework::standard::CommandError as Error;
use serenity::{
    framework::standard::{macros::command, Args, CommandResult},
    model::{
        channel::{Message, Reaction, ReactionType},
        id::UserId,
    },
    utils::MessageBuilder,
};
use std::collections::{HashMap as Map, HashSet as Set};
use std::time::Duration;
use youmubot_prelude::{Duration as ParseDuration, *};

#[command]
#[description = "🎌 Cast a poll upon everyone and ask them for opinions!"]
#[usage = "[duration] / [question] / [answer #1 = Yes!] / [answer #2 = No!] ..."]
#[example = "2m/How early do you get up?/Before 6/Before 7/Before 8/Fuck time"]
#[bucket = "voting"]
#[only_in(guilds)]
#[min_args(2)]
#[owner_privilege]
pub fn vote(ctx: &mut Context, msg: &Message, mut args: Args) -> CommandResult {
    // Parse stuff first
    let args = args.quoted();
    let _duration = args.single::<ParseDuration>()?;
    let duration = &_duration.0;
    if *duration < Duration::from_secs(2 * 60) || *duration > Duration::from_secs(60 * 60 * 24) {
        msg.reply(ctx, format!("😒 Invalid duration ({}). The voting time should be between **2 minutes** and **1 day**.", _duration))?;
        return Ok(());
    }
    let question = args.single::<String>()?;
    let choices = if args.is_empty() {
        vec![
            ("😍".to_owned(), "Yes! 😍".to_owned()),
            ("🤢".to_owned(), "No! 🤢".to_owned()),
        ]
    } else {
        let choices: Vec<_> = args.iter().map(|v| v.unwrap()).collect();
        if choices.len() < 2 {
            // Where are the choices?
            msg.reply(
                ctx,
                "😒 Can't have a nice voting session if you only have one choice.",
            )?;
            return Ok(());
        }
        if choices.len() > MAX_CHOICES {
            // Too many choices!
            msg.reply(
                ctx,
                format!(
                    "😵 Too many choices... We only support {} choices at the moment!",
                    MAX_CHOICES
                ),
            )?;
            return Ok(());
        }

        pick_n_reactions(choices.len())?
            .into_iter()
            .zip(choices.into_iter())
            .collect()
    };

    let fields: Vec<_> = {
        choices
            .iter()
            .map(|(reaction, choice)| {
                (
                    MessageBuilder::new().push_bold_safe(choice).build(),
                    format!("React with {}", reaction),
                    true,
                )
            })
            .collect()
    };

    // Ok... now we post up a nice voting panel.
    let channel = msg.channel_id;
    let author = msg.author.clone();
    let panel = channel.send_message(&ctx, |c| {
        c.content("@here").embed(|e| {
            e.author(|au| {
                au.icon_url(author.avatar_url().unwrap_or("".to_owned()))
                    .name(&author.name)
            })
            .title(format!("You have {} to vote!", _duration))
            .thumbnail("https://images-ext-2.discordapp.net/external/BK7injOyt4XT8yNfbCDV4mAkwoRy49YPfq-3IwCc_9M/http/cdn.i.ntere.st/p/9197498/image")
            .description(MessageBuilder::new().push_bold_line_safe(&question).push("\nThis question was asked by ").push(author.mention()))
            .fields(fields.into_iter())
        })
    })?;
    msg.delete(&ctx)?;
    // React on all the choices
    choices
        .iter()
        .try_for_each(|(emote, _)| panel.react(&ctx, &emote[..]))?;

    // A handler for votes.
    struct VoteHandler {
        pub ctx: Context,
        pub msg: Message,
        pub user_reactions: Map<String, Set<UserId>>,

        pub panel: Message,
    }

    impl VoteHandler {
        fn new(ctx: Context, msg: Message, panel: Message, choices: &[(String, String)]) -> Self {
            VoteHandler {
                ctx,
                msg,
                user_reactions: choices
                    .iter()
                    .map(|(emote, _)| (emote.clone(), Set::new()))
                    .collect(),
                panel,
            }
        }
    }

    impl ReactionHandler for VoteHandler {
        fn handle_reaction(&mut self, reaction: &Reaction, is_add: bool) -> CommandResult {
            if reaction.message_id != self.panel.id {
                return Ok(());
            }
            if reaction.user(&self.ctx)?.bot {
                return Ok(());
            }
            let users = if let ReactionType::Unicode(ref s) = reaction.emoji {
                if let Some(users) = self.user_reactions.get_mut(s.as_str()) {
                    users
                } else {
                    return Ok(());
                }
            } else {
                return Ok(());
            };
            if is_add {
                users.insert(reaction.user_id);
            } else {
                users.remove(&reaction.user_id);
            }
            Ok(())
        }
    }

    ctx.data
        .get_cloned::<ReactionWatcher>()
        .handle_reactions_timed(
            VoteHandler::new(ctx.clone(), msg.clone(), panel, &choices),
            *duration,
            move |vh| {
                let (ctx, msg, user_reactions, panel) =
                    (vh.ctx, vh.msg, vh.user_reactions, vh.panel);
                let choice_map = choices.into_iter().collect::<Map<_, _>>();
                let result: Vec<(String, Vec<UserId>)> = user_reactions
                    .into_iter()
                    .filter(|(_, users)| !users.is_empty())
                    .map(|(emote, users)| (emote, users.into_iter().collect()))
                    .collect();

                if result.len() == 0 {
                    msg.reply(
                        &ctx,
                        MessageBuilder::new()
                            .push("no one answer your question ")
                            .push_bold_safe(&question)
                            .push(", sorry 😭")
                            .build(),
                    )
                    .ok();
                } else {
                    channel
                        .send_message(&ctx, |c| {
                            c.content({
                                let mut content = MessageBuilder::new();
                                content
                                    .push("@here, ")
                                    .push(author.mention())
                                    .push(" previously asked ")
                                    .push_bold_safe(&question)
                                    .push(", and here are the results!");
                                result.into_iter().for_each(|(emote, votes)| {
                                    content
                                        .push("\n - ")
                                        .push_bold(format!("{}", votes.len()))
                                        .push(" voted for ")
                                        .push(&emote)
                                        .push(" ")
                                        .push_bold_safe(choice_map.get(&emote).unwrap())
                                        .push(": ")
                                        .push(
                                            votes
                                                .into_iter()
                                                .map(|v| v.mention())
                                                .collect::<Vec<_>>()
                                                .join(", "),
                                        );
                                });
                                content.build()
                            })
                        })
                        .ok();
                }
                panel.delete(&ctx).ok();
            },
        );

    Ok(())
    // unimplemented!();
}

// Pick a set of random n reactions!
fn pick_n_reactions(n: usize) -> Result<Vec<String>, Error> {
    use rand::seq::SliceRandom;
    if n > MAX_CHOICES {
        Err(Error::from("Too many options"))
    } else {
        let mut rand = rand::thread_rng();
        Ok(REACTIONS
            .choose_multiple(&mut rand, n)
            .map(|v| (*v).to_owned())
            .collect())
    }
}

const MAX_CHOICES: usize = 15;

// All the defined reactions.
const REACTIONS: [&'static str; 90] = [
    "😀", "😁", "😂", "🤣", "😃", "😄", "😅", "😆", "😉", "😊", "😋", "😎", "😍", "😘", "🥰", "😗",
    "😙", "😚", "☺️", "🙂", "🤗", "🤩", "🤔", "🤨", "😐", "😑", "😶", "🙄", "😏", "😣", "😥", "😮",
    "🤐", "😯", "😪", "😫", "😴", "😌", "😛", "😜", "😝", "🤤", "😒", "😓", "😔", "😕", "🙃", "🤑",
    "😲", "☹️", "🙁", "😖", "😞", "😟", "😤", "😢", "😭", "😦", "😧", "😨", "😩", "🤯", "😬", "😰",
    "😱", "🥵", "🥶", "😳", "🤪", "😵", "😡", "😠", "🤬", "😷", "🤒", "🤕", "🤢", "🤮", "🤧", "😇",
    "🤠", "🤡", "🥳", "🥴", "🥺", "🤥", "🤫", "🤭", "🧐", "🤓",
];

// Assertions
static_assertions::const_assert!(MAX_CHOICES <= REACTIONS.len());
