use serde::Deserialize;
use serenity::builder::EditMessage;
use serenity::framework::standard::CommandError as Error;
use serenity::{
    framework::standard::{
        macros::{check, command},
        Args, CommandOptions, CommandResult, Reason,
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
async fn nsfw_check(
    ctx: &Context,
    msg: &Message,
    _: &mut Args,
    _: &CommandOptions,
) -> Result<(), Reason> {
    let channel = msg.channel_id.to_channel(&ctx).await.unwrap();
    if !(match channel {
        Channel::Guild(guild_channel) => guild_channel.nsfw,
        _ => true,
    }) {
        Err(Reason::User("😣 YOU FREAKING PERVERT!!!".to_owned()))
    } else {
        Ok(())
    }
}

async fn message_command(
    ctx: &Context,
    msg: &Message,
    args: Args,
    rating: Rating,
) -> CommandResult {
    let tags = args.remains().unwrap_or("touhou");
    let images = get_image(
        ctx.data.read().await.get::<HTTPClient>().unwrap(),
        rating,
        tags,
    )
    .await?;
    if images.is_empty() {
        msg.reply(&ctx, "🖼️ No image found...\n💡 Tip: In danbooru, character names follow Japanese standards (last name before first name), so **Hakurei Reimu** might give you an image while **Reimu Hakurei** won't.").await?;
        return Ok(());
    }
    let images = std::sync::Arc::new(images);
    paginate_reply(
        paginate_from_fn(|page, ctx, msg: &mut Message| {
            let images = images.clone();
            Box::pin(async move {
                let page = page as usize;
                if page >= images.len() {
                    Ok(false)
                } else {
                    msg.edit(
                        ctx,
                        EditMessage::new().content(format!(
                            "[🖼️  **{}/{}**] Here's the image you requested!\n\n{}",
                            page + 1,
                            images.len(),
                            images[page]
                        )),
                    )
                    .await
                    .map(|_| true)
                    .map_err(|e| e.into())
                }
            })
        })
        .with_page_count(images.len()),
        ctx,
        msg,
        std::time::Duration::from_secs(120),
    )
    .await?;
    Ok(())
}

// Gets an image URL.
async fn get_image(
    client: &<HTTPClient as TypeMapKey>::Value,
    rating: Rating,
    tags: &str,
) -> Result<Vec<String>, Error> {
    // Fix the tags: change whitespaces to +
    let tags = tags.split_whitespace().collect::<Vec<_>>().join("_");
    let req = client
        .get(format!(
            "https://danbooru.donmai.us/posts.json?tags=rating:{}+{}",
            rating.to_string(),
            tags
        ))
        .query(&[("limit", "50"), ("random", "true")])
        .build()?;
    let response: Vec<PostResponse> = client.execute(req).await?.json().await?;
    Ok(response
        .into_iter()
        .filter_map(|v| {
            v.id.map(|id| format!("https://danbooru.donmai.us/posts/{}", id))
        })
        .collect())
}

#[derive(Deserialize, Debug)]
struct PostResponse {
    id: Option<u64>,
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
