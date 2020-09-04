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

/// Paginate! with a pager function.
/// If awaited, will block until everything is done.
pub async fn paginate<'a, T, F>(
    mut pager: T,
    ctx: &'a Context,
    channel: ChannelId,
    timeout: std::time::Duration,
) -> Result<()>
where
    T: for<'m> FnMut(u8, &'a Context, &'m mut Message) -> F,
    F: Future<Output = Result<bool>>,
{
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
    // Build a reaction collector
    let mut reaction_collector = message.await_reactions(&ctx).await;
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
async fn handle_reaction<'a, T, F>(
    page: u8,
    pager: &mut T,
    ctx: &'a Context,
    message: &'_ mut Message,
    reaction: &ReactionAction,
) -> Result<u8>
where
    T: for<'m> FnMut(u8, &'a Context, &'m mut Message) -> F,
    F: Future<Output = Result<bool>>,
{
    let reaction = match reaction {
        ReactionAction::Added(v) | ReactionAction::Removed(v) => v,
    };
    match &reaction.emoji {
        ReactionType::Unicode(ref s) => match s.as_str() {
            ARROW_LEFT if page == 0 => Ok(page),
            ARROW_LEFT => Ok(if pager(page - 1, ctx, message).await? {
                page - 1
            } else {
                page
            }),
            ARROW_RIGHT => Ok(if pager(page + 1, ctx, message).await? {
                page + 1
            } else {
                page
            }),
            _ => Ok(page),
        },
        _ => Ok(page),
    }
}
