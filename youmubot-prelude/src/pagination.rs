use crate::{Context, Result};
use futures_util::{future::Future, StreamExt};
use serenity::{
    collector::ReactionAction,
    model::{
        channel::{Message, ReactionType},
        id::ChannelId,
    },
};
use std::convert::TryFrom;
use tokio::time as tokio_time;

const ARROW_RIGHT: &'static str = "➡️";
const ARROW_LEFT: &'static str = "⬅️";

#[async_trait::async_trait]
pub trait Paginate {
    async fn render(&mut self, page: u8, ctx: &Context, m: &mut Message) -> Result<bool>;
}

#[async_trait::async_trait]
impl<T> Paginate for T
where
    T: for<'m> FnMut(
            u8,
            &'m Context,
            &'m mut Message,
        ) -> std::pin::Pin<Box<dyn Future<Output = Result<bool>> + Send + 'm>>
        + Send,
{
    async fn render(&mut self, page: u8, ctx: &Context, m: &mut Message) -> Result<bool> {
        self(page, ctx, m).await
    }
}

// Paginate! with a pager function.
/// If awaited, will block until everything is done.
pub async fn paginate(
    mut pager: impl for<'m> FnMut(
            u8,
            &'m Context,
            &'m mut Message,
        ) -> std::pin::Pin<Box<dyn Future<Output = Result<bool>> + Send + 'm>>
        + Send,
    ctx: &Context,
    channel: ChannelId,
    timeout: std::time::Duration,
) -> Result<()> {
    let mut message = channel
        .send_message(&ctx, |e| e.content("Youmu is loading the first page..."))
        .await?;
    // React to the message
    message
        .react(&ctx, ReactionType::try_from(ARROW_LEFT)?)
        .await?;
    message
        .react(&ctx, ReactionType::try_from(ARROW_RIGHT)?)
        .await?;
    pager(0, ctx, &mut message).await?;
    // Build a reaction collector
    let mut reaction_collector = message.await_reactions(&ctx).removed(true).await;
    let mut page = 0;

    // Loop the handler function.
    let res: Result<()> = loop {
        match tokio_time::timeout(timeout, reaction_collector.next()).await {
            Err(_) => break Ok(()),
            Ok(None) => break Ok(()),
            Ok(Some(reaction)) => {
                page = match handle_reaction(page, &mut pager, ctx, &mut message, &reaction).await {
                    Ok(v) => v,
                    Err(e) => break Err(e),
                };
            }
        }
    };

    message.react(&ctx, '🛑').await?;

    res
}

// Handle the reaction and return a new page number.
async fn handle_reaction(
    page: u8,
    pager: &mut impl Paginate,
    ctx: &Context,
    message: &mut Message,
    reaction: &ReactionAction,
) -> Result<u8> {
    let reaction = match reaction {
        ReactionAction::Added(v) | ReactionAction::Removed(v) => v,
    };
    match &reaction.emoji {
        ReactionType::Unicode(ref s) => match s.as_str() {
            ARROW_LEFT if page == 0 => Ok(page),
            ARROW_LEFT => Ok(if pager.render(page - 1, ctx, message).await? {
                page - 1
            } else {
                page
            }),
            ARROW_RIGHT => Ok(if pager.render(page + 1, ctx, message).await? {
                page + 1
            } else {
                page
            }),
            _ => Ok(page),
        },
        _ => Ok(page),
    }
}
