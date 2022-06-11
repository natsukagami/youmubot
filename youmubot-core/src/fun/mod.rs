use rand::{
    distributions::{Distribution, Uniform},
    thread_rng,
};
use serenity::{
    framework::standard::{
        macros::{command, group},
        Args, CommandResult,
    },
    model::{channel::Message, id::UserId},
    utils::MessageBuilder,
};
use youmubot_prelude::*;

mod images;
mod names;

use images::*;

#[group]
#[description = "Random commands"]
#[commands(roll, pick, name, image, nsfw)]
struct Fun;

#[command]
#[description = "🎲 Rolls a dice that gives you a random number."]
#[min_args(0)]
#[max_args(2)]
#[usage = "[max-dice-faces = 6] / [message]"]
#[example = "100 / What's my score?"]
async fn roll(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let dice = if args.is_empty() {
        6
    } else {
        args.single::<u64>()?
    };

    if dice == 0 {
        msg.reply(&ctx, "Give me a dice with 0 faces, what do you expect 😒")
            .await?;
        return Ok(());
    }

    let result = {
        let dice_rng = Uniform::from(1..=dice);
        let mut rng = thread_rng();
        dice_rng.sample(&mut rng)
    };

    match args.single_quoted::<String>() {
        Ok(s) => {
            msg.reply(
                &ctx,
                MessageBuilder::new()
                    .push("you asked ")
                    .push_bold_safe(s)
                    .push(format!(
                        ", so I rolled a 🎲 of **{}** faces, and got **{}**!",
                        dice, result
                    ))
                    .build(),
            )
            .await
        }
        Err(_) if args.is_empty() => {
            msg.reply(
                &ctx,
                format!(
                    "I rolled a 🎲 of **{}** faces, and got **{}**!",
                    dice, result
                ),
            )
            .await
        }
        Err(e) => return Err(e.into()),
    }?;

    Ok(())
}

#[command]
#[description = r#"👈 Pick a choice from the available list of choices. 
You may prefix the first choice with `?` to make it a question!
If no choices are given, Youmu defaults to `Yes!` and `No!`"#]
#[usage = "[?question]/[choice #1]/[choice #2]/..."]
#[example = "?What for dinner/Pizza/Hamburger"]
async fn pick(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let (question, choices) = {
        // Get a list of options.
        let mut choices = args
            .quoted()
            .trimmed()
            .iter::<String>()
            .map(|v| v.unwrap())
            .peekable();
        // If we have the first argument as question, use it.
        let question = match choices.peek() {
            Some(ref q) if q.starts_with('?') => Some(q.replacen("?", "", 1) + "?"),
            _ => None,
        };
        // If we have a question, that's not a choice.
        let mut choices = match question {
            Some(_) => {
                choices.next();
                choices
            }
            None => choices,
        };
        // If there are no choices, default to Yes! and No!
        let choices = match choices.peek() {
            None => vec!["Yes!".to_owned(), "No!".to_owned()],
            _ => choices.collect(),
        };
        (question, choices)
    };

    let choice = {
        let uniform = Uniform::from(0..choices.len());
        let mut rng = thread_rng();
        &choices[uniform.sample(&mut rng)]
    };

    match question {
        None => {
            msg.reply(
                &ctx,
                MessageBuilder::new()
                    .push("Youmu picks 👉")
                    .push_bold_safe(choice)
                    .push("👈!")
                    .build(),
            )
            .await
        }
        Some(s) => {
            msg.reply(
                &ctx,
                MessageBuilder::new()
                    .push("you asked ")
                    .push_bold_safe(s)
                    .push(", and Youmu picks 👉")
                    .push_bold_safe(choice)
                    .push("👈!")
                    .build(),
            )
            .await
        }
    }?;

    Ok(())
}

#[command]
#[description = "Wanna know what your name is in Japanese🇯🇵?"]
#[usage = "[user_mention = yourself]"]
#[example = "@user#1234"]
#[max_args(1)]
async fn name(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let user_id = if args.is_empty() {
        msg.author.id
    } else {
        args.single::<UserId>()?
    };

    let user_mention = if user_id == msg.author.id {
        "your".to_owned()
    } else {
        MessageBuilder::new()
            .push_bold_safe(user_id.to_user(&ctx).await?.tag())
            .push("'s")
            .build()
    };

    // Rule out a couple of cases
    if user_id == ctx.http.get_current_user().await?.id {
        // This is my own user_id
        msg.reply(&ctx, "😠 My name is **Youmu Konpaku**!").await?;
        return Ok(());
    }

    let (first_name, last_name) = names::name_from_userid(user_id);

    msg.reply(
        &ctx,
        format!(
            "{} Japanese🇯🇵 name is **{} {}**!",
            user_mention, first_name, last_name
        ),
    )
    .await?;
    Ok(())
}
