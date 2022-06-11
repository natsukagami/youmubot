use serenity::framework::standard::CommandError as Error;
use serenity::{
    collector::ReactionAction,
    framework::standard::{macros::command, Args, CommandResult},
    model::{
        channel::{Message, ReactionType},
        id::UserId,
    },
    utils::MessageBuilder,
};
use std::time::Duration;
use std::{
    collections::{HashMap as Map, HashSet as Set},
    convert::TryFrom,
};
use youmubot_prelude::{Duration as ParseDuration, *};

#[command]
#[description = "🎌 Cast a poll upon everyone and ask them for opinions!"]
#[usage = "[duration] / [question] / [answer #1 = Yes!] / [answer #2 = No!] ..."]
#[example = "2m/How early do you get up?/Before 6/Before 7/Before 8/Fuck time"]
#[bucket = "voting"]
#[only_in(guilds)]
#[min_args(2)]
#[owner_privilege]
pub async fn vote(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    // Parse stuff first
    let args = args.quoted();
    let _duration = args.single::<ParseDuration>()?;
    let duration = &_duration.0;
    if *duration < Duration::from_secs(2) || *duration > Duration::from_secs(60 * 60 * 24) {
        msg.reply(ctx, format!("😒 Invalid duration ({}). The voting time should be between **2 minutes** and **1 day**.", _duration)).await?;
        return Ok(());
    }
    let question = args.single::<String>()?;
    let choices = if args.is_empty() {
        vec![
            ("😍".to_owned(), "Yes! 😍".to_owned()),
            ("🤢".to_owned(), "No! 🤢".to_owned()),
        ]
    } else {
        let choices: Vec<_> = args.iter().quoted().trimmed().map(|v| v.unwrap()).collect();
        if choices.len() < 2 {
            // Where are the choices?
            msg.reply(
                ctx,
                "😒 Can't have a nice voting session if you only have one choice.",
            )
            .await?;
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
            )
            .await?;
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
    let asked = msg.timestamp;
    let until = *asked + (chrono::Duration::from_std(*duration).unwrap());
    let panel = channel.send_message(&ctx, |c| {
        c.content("@here").embed(|e| {
            e.author(|au| {
                au.icon_url(author.avatar_url().unwrap_or_else(|| "".to_owned()))
                    .name(&author.name)
            })
            .title(format!("Please vote! Poll ends {}", until.format("<t:%s:R>")))
            .thumbnail("https://images-ext-2.discordapp.net/external/BK7injOyt4XT8yNfbCDV4mAkwoRy49YPfq-3IwCc_9M/http/cdn.i.ntere.st/p/9197498/image")
            .description(MessageBuilder::new().push_bold_line_safe(&question).push("\nThis question was asked by ").push(author.mention()))
            .fields(fields.into_iter())
        })
    }).await?;
    msg.delete(&ctx).await?;

    // React on all the choices
    for (emote, _) in &choices {
        panel
            .react(&ctx, ReactionType::try_from(&emote[..]).unwrap())
            .map_ok(|_| ())
            .await?;
    }

    // A handler for votes.
    let user_reactions: Map<String, Set<UserId>> = choices
        .iter()
        .map(|(emote, _)| (emote.clone(), Set::new()))
        .collect();

    // Collect reactions...
    let user_reactions = panel
        .await_reactions(&ctx)
        .removed(true)
        .timeout(*duration)
        .build()
        .fold(user_reactions, |mut set, reaction| async move {
            let (reaction, is_add) = match &*reaction {
                ReactionAction::Added(r) => (r, true),
                ReactionAction::Removed(r) => (r, false),
            };
            let users = if let ReactionType::Unicode(ref s) = reaction.emoji {
                if let Some(users) = set.get_mut(s.as_str()) {
                    users
                } else {
                    return set;
                }
            } else {
                return set;
            };
            let user_id = match reaction.user_id {
                Some(v) => v,
                None => return set,
            };
            if is_add {
                users.insert(user_id);
            } else {
                users.remove(&user_id);
            }
            set
        })
        .await;

    // Handle choices
    let choice_map = choices.into_iter().collect::<Map<_, _>>();
    let mut result: Vec<(String, Vec<UserId>)> = user_reactions
        .into_iter()
        .filter(|(_, users)| !users.is_empty())
        .map(|(emote, users)| (emote, users.into_iter().collect()))
        .collect();

    result.sort_unstable_by(|(_, v), (_, w)| w.len().cmp(&v.len()));

    if result.is_empty() {
        msg.reply(
            &ctx,
            MessageBuilder::new()
                .push("no one answer your question ")
                .push_bold_safe(&question)
                .push(", sorry 😭")
                .build(),
        )
        .await?;
        return Ok(());
    }

    channel
        .send_message(&ctx, |c| {
            c.content({
                let mut content = MessageBuilder::new();
                content
                    .push("@here, ")
                    .push(asked.format("<t:%s:R>, "))
                    .push(author.mention())
                    .push(" asked ")
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
                                .map(|v| v.mention().to_string())
                                .collect::<Vec<_>>()
                                .join(", "),
                        );
                });
                content.build()
            })
        })
        .await?;
    panel.delete(&ctx).await?;

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
const REACTIONS: [&str; 90] = [
    "😀", "😁", "😂", "🤣", "😃", "😄", "😅", "😆", "😉", "😊", "😋", "😎", "😍", "😘", "🥰", "😗",
    "😙", "😚", "☺️", "🙂", "🤗", "🤩", "🤔", "🤨", "😐", "😑", "😶", "🙄", "😏", "😣", "😥", "😮",
    "🤐", "😯", "😪", "😫", "😴", "😌", "😛", "😜", "😝", "🤤", "😒", "😓", "😔", "😕", "🙃", "🤑",
    "😲", "☹️", "🙁", "😖", "😞", "😟", "😤", "😢", "😭", "😦", "😧", "😨", "😩", "🤯", "😬", "😰",
    "😱", "🥵", "🥶", "😳", "🤪", "😵", "😡", "😠", "🤬", "😷", "🤒", "🤕", "🤢", "🤮", "🤧", "😇",
    "🤠", "🤡", "🥳", "🥴", "🥺", "🤥", "🤫", "🤭", "🧐", "🤓",
];

// Assertions
static_assertions::const_assert!(MAX_CHOICES <= REACTIONS.len());
static_assertions::const_assert!(MAX_CHOICES <= REACTIONS.len());
