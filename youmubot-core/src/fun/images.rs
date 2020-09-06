use serde::Deserialize;
use serenity::framework::standard::CommandError as Error;
use serenity::{
    framework::standard::{
        macros::{check, command},
        Args, CheckResult, CommandOptions, CommandResult, Reason,
    },
    model::channel::{Channel, Message},
};
use std::string::ToString;
use youmubot_prelude::*;

#[command]
#[checks(nsfw)]
#[description = "🖼️ Find an image with a given tag on Danbooru[nsfw]!"]
#[min_args(1)]
#[bucket("images")]
pub async fn nsfw(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    message_command(ctx, msg, args, Rating::Explicit).await
}

#[command]
#[description = "🖼️ Find an image with a given tag on Danbooru[safe]!"]
#[min_args(1)]
#[bucket("images")]
pub async fn image(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    message_command(ctx, msg, args, Rating::Safe).await
}

#[check]
#[name = "nsfw"]
async fn nsfw_check(ctx: &Context, msg: &Message, _: &mut Args, _: &CommandOptions) -> CheckResult {
    let channel = msg.channel_id.to_channel(&ctx).await.unwrap();
    if !(match channel {
        Channel::Guild(guild_channel) => guild_channel.nsfw,
        _ => true,
    }) {
        CheckResult::Failure(Reason::User("😣 YOU FREAKING PERVERT!!!".to_owned()))
    } else {
        CheckResult::Success
    }
}

async fn message_command(
    ctx: &Context,
    msg: &Message,
    args: Args,
    rating: Rating,
) -> CommandResult {
    let tags = args.remains().unwrap_or("touhou");
    let image = get_image(
        ctx.data.read().await.get::<HTTPClient>().unwrap(),
        rating,
        tags,
    )
    .await?;
    match image {
        None => msg.reply(&ctx, "🖼️ No image found...\n💡 Tip: In danbooru, character names follow Japanese standards (last name before first name), so **Hakurei Reimu** might give you an image while **Reimu Hakurei** won't.").await,
        Some(url) => msg.reply(
            &ctx,
            format!("🖼️ Here's the image you requested!\n\n{}", url),
        ).await,
    }?;
    Ok(())
}

// Gets an image URL.
async fn get_image(
    client: &<HTTPClient as TypeMapKey>::Value,
    rating: Rating,
    tags: &str,
) -> Result<Option<String>, Error> {
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
    let response: Vec<PostResponse> = client.execute(req).await?.json().await?;
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
