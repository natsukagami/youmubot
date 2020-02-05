use crate::commands::args::Duration as ParseDuration;
use serenity::framework::standard::CommandError as Error;
use serenity::{
    framework::standard::{macros::command, Args, CommandResult},
    model::{
        channel::{Message, MessageReaction, ReactionType},
        id::UserId,
    },
    utils::MessageBuilder,
};
use std::collections::HashMap as Map;
use std::thread;
use std::time::Duration;
use youmubot_prelude::*;

#[command]
#[description = "🎌 Cast a poll upon everyone and ask them for opinions!"]
#[usage = "[duration] / [question] / [answer #1 = Yes!] / [answer #2 = No!] ..."]
#[example = "2m/How early do you get up?/Before 6/Before 7/Before 8/Fuck time"]
#[bucket = "voting"]
#[only_in(guilds)]
#[min_args(2)]
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
        vec![("😍", "Yes! 😍".to_owned()), ("🤢", "No! 🤢".to_owned())]
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
            .map(|(choice, reaction)| {
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
    let author = &msg.author;
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
        .try_for_each(|(v, _)| panel.react(&ctx, *v))?;

    // Start sleeping
    thread::sleep(*duration);

    let result = collect_reactions(ctx, &panel, &choices)?;
    if result.len() == 0 {
        msg.reply(
            &ctx,
            MessageBuilder::new()
                .push("no one answer your question ")
                .push_bold_safe(&question)
                .push(", sorry 😭")
                .build(),
        )?;
    } else {
        channel.send_message(&ctx, |c| {
            c.content({
                let mut content = MessageBuilder::new();
                content
                    .push("@here, ")
                    .push(author.mention())
                    .push(" previously asked ")
                    .push_bold_safe(&question)
                    .push(", and here are the results!");
                result.iter().for_each(|(choice, votes)| {
                    content
                        .push("\n - ")
                        .push_bold(format!("{}", votes.len()))
                        .push(" voted for ")
                        .push_bold_safe(choice)
                        .push(": ")
                        .push(
                            votes
                                .iter()
                                .map(|v| v.mention())
                                .collect::<Vec<_>>()
                                .join(", "),
                        );
                });
                content.build()
            })
        })?;
    }
    panel.delete(&ctx)?;

    Ok(())
    // unimplemented!();
}

// Collect reactions and store them as a map from choice to
fn collect_reactions<'a>(
    ctx: &mut Context,
    msg: &Message,
    choices: &'a [(&'static str, String)],
) -> Result<Vec<(&'a str, Vec<UserId>)>, Error> {
    // Get a brand new version of the Message
    let reactions = msg.channel_id.message(&ctx, msg.id)?.reactions;
    let reaction_to_choice: Map<_, _> = choices.into_iter().map(|r| (r.0, &r.1)).collect();
    let mut vec: Vec<(&str, Vec<UserId>)> = Vec::new();
    reactions
        .into_iter()
        .filter_map(|r| {
            if let ReactionType::Unicode(ref v) = r.reaction_type {
                reaction_to_choice
                    .get(&&v[..])
                    .cloned()
                    .filter(|_| r.count > 1)
                    .map(|choice| (r.clone(), choice))
            } else {
                None
            }
        })
        .try_for_each(|(r, choice)| -> Result<_, Error> {
            let users = collect_reaction_users(ctx, &msg, &r)?;
            vec.push((choice, users));
            Ok(())
        })?;
    vec.sort_by(|(_, b): &(_, Vec<_>), (_, d)| d.len().cmp(&b.len()));
    Ok(vec)
}

fn collect_reaction_users(
    ctx: &mut Context,
    msg: &Message,
    reaction: &MessageReaction,
) -> Result<Vec<UserId>, Error> {
    let mut res = Vec::with_capacity(reaction.count as usize);
    (0..reaction.count)
        .step_by(100)
        .try_for_each(|_| -> Result<_, Error> {
            let user_ids = msg
                .reaction_users(
                    &ctx,
                    reaction.reaction_type.clone(),
                    Some(100),
                    res.last().cloned(),
                )?
                .into_iter()
                .map(|i| i.id);
            res.extend(user_ids);
            Ok(())
        })?;
    Ok(res)
}
// Pick a set of random n reactions!
fn pick_n_reactions(n: usize) -> Result<Vec<&'static str>, Error> {
    use rand::seq::SliceRandom;
    if n > MAX_CHOICES {
        Err(Error::from("Too many options"))
    } else {
        let mut rand = rand::thread_rng();
        Ok(REACTIONS
            .choose_multiple(&mut rand, n)
            .map(|v| *v)
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
