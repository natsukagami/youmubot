use crate::http::HTTP;
use reqwest::blocking::Client as HTTPClient;
use serde::Deserialize;
use serenity::framework::standard::CommandError as Error;
use serenity::prelude::*;
use serenity::{
    framework::standard::{
        macros::{check, command},
        Args, CheckResult, CommandOptions, CommandResult, Reason,
    },
    model::channel::{Channel, Message},
};
use std::string::ToString;

#[command]
#[checks(nsfw)]
#[description = "🖼️ Find an image with a given tag on Danbooru[nsfw]!"]
#[min_args(1)]
#[bucket("images")]
pub fn nsfw(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    message_command(ctx, msg, args, Rating::Explicit)
}

#[command]
#[description = "🖼️ Find an image with a given tag on Danbooru[safe]!"]
#[min_args(1)]
#[bucket("images")]
pub fn image(ctx: &mut Context, msg: &Message, args: Args) -> CommandResult {
    message_command(ctx, msg, args, Rating::Safe)
}

#[check]
#[name = "nsfw"]
fn nsfw_check(ctx: &mut Context, msg: &Message, _: &mut Args, _: &CommandOptions) -> CheckResult {
    let channel = msg.channel_id.to_channel(&ctx).unwrap();
    if !(match channel {
        Channel::Guild(guild_channel) => guild_channel.read().nsfw,
        _ => true,
    }) {
        CheckResult::Failure(Reason::User("😣 YOU FREAKING PERVERT!!!".to_owned()))
    } else {
        CheckResult::Success
    }
}

fn message_command(ctx: &mut Context, msg: &Message, args: Args, rating: Rating) -> CommandResult {
    let tags = args.remains().unwrap_or("touhou");
    let http = ctx.data.read();
    let http = http.get::<HTTP>().unwrap();
    let image = get_image(http, rating, tags)?;
    match image {
        None => msg.reply(&ctx, "🖼️ No image found...\n💡 Tip: In danbooru, character names follow Japanese standards (last name before first name), so **Hakurei Reimu** might give you an image while **Reimu Hakurei** won't."),
        Some(url) => msg.reply(
            &ctx,
            format!("🖼️ Here's the image you requested!\n\n{}", url),
        ),
    }?;
    Ok(())
}

// Gets an image URL.
fn get_image(client: &HTTPClient, rating: Rating, tags: &str) -> Result<Option<String>, Error> {
    // Fix the tags: change whitespaces to +
    let tags = tags.split_whitespace().collect::<Vec<_>>().join("_");
    let req = client
        .get(&format!(
            "https://danbooru.donmai.us/posts.json?tags=rating:{}+{}",
            rating.to_string(),
            tags
        ))
        .query(&[("limit", "1"), ("random", "true")])
        .build()?;
    println!("{:?}", req.url());
    let response: Vec<PostResponse> = client.execute(req)?.json()?;
    Ok(response
        .into_iter()
        .next()
        .map(|v| format!("https://danbooru.donmai.us/posts/{}", v.id)))
}

#[derive(Deserialize, Debug)]
struct PostResponse {
    id: u64,
}

#[derive(Copy, Clone, Debug)]
enum Rating {
    Explicit,
    Safe,
}

impl ToString for Rating {
    fn to_string(&self) -> String {
        use Rating::*;
        match self {
            Explicit => "explicit",
            Safe => "safe",
        }
        .to_owned()
    }
}
