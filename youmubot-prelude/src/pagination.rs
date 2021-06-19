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

const ARROW_RIGHT: &str = "➡️";
const ARROW_LEFT: &str = "⬅️";

/// A trait that provides the implementation of a paginator.
#[async_trait::async_trait]
pub trait Paginate: Send + Sized {
    /// Render the given page.
    async fn render(&mut self, page: u8, ctx: &Context, m: &mut Message) -> Result<bool>;

    /// Any setting-up before the rendering stage.
    async fn prerender(&mut self, _ctx: &Context, _m: &mut Message) -> Result<()> {
        Ok(())
    }

    /// Handle the incoming reaction. Defaults to calling `handle_pagination_reaction`, but you can do some additional handling
    /// before handing the functionality over.
    ///
    /// Return the resulting current page, or `None` if the pagination should stop.
    async fn handle_reaction(
        &mut self,
        page: u8,
        ctx: &Context,
        message: &mut Message,
        reaction: &ReactionAction,
    ) -> Result<Option<u8>> {
        handle_pagination_reaction(page, self, ctx, message, reaction)
            .await
            .map(Some)
    }
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

// Paginate! with a pager function, and replying to a message.
/// If awaited, will block until everything is done.
pub async fn paginate_reply(
    pager: impl Paginate,
    ctx: &Context,
    reply_to: &Message,
    timeout: std::time::Duration,
) -> Result<()> {
    let message = reply_to
        .reply(&ctx, "Youmu is loading the first page...")
        .await?;
    paginate_with_first_message(pager, ctx, message, timeout).await
}

// Paginate! with a pager function.
/// If awaited, will block until everything is done.
pub async fn paginate(
    pager: impl Paginate,
    ctx: &Context,
    channel: ChannelId,
    timeout: std::time::Duration,
) -> Result<()> {
    let message = channel
        .send_message(&ctx, |e| e.content("Youmu is loading the first page..."))
        .await?;
    paginate_with_first_message(pager, ctx, message, timeout).await
}

async fn paginate_with_first_message(
    mut pager: impl Paginate,
    ctx: &Context,
    mut message: Message,
    timeout: std::time::Duration,
) -> Result<()> {
    // React to the message
    message
        .react(&ctx, ReactionType::try_from(ARROW_LEFT)?)
        .await?;
    message
        .react(&ctx, ReactionType::try_from(ARROW_RIGHT)?)
        .await?;
    pager.prerender(&ctx, &mut message).await?;
    pager.render(0, ctx, &mut message).await?;
    // Build a reaction collector
    let mut reaction_collector = message.await_reactions(&ctx).removed(true).await;
    let mut page = 0;

    // Loop the handler function.
    let res: Result<()> = loop {
        match tokio_time::timeout(timeout, reaction_collector.next()).await {
            Err(_) => break Ok(()),
            Ok(None) => break Ok(()),
            Ok(Some(reaction)) => {
                page = match pager
                    .handle_reaction(page, ctx, &mut message, &reaction)
                    .await
                {
                    Ok(Some(v)) => v,
                    Ok(None) => break Ok(()),
                    Err(e) => break Err(e),
                };
            }
        }
    };

    message.react(&ctx, '🛑').await?;

    res
}

/// Same as `paginate`, but for function inputs, especially anonymous functions.
pub async fn paginate_fn(
    pager: impl for<'m> FnMut(
            u8,
            &'m Context,
            &'m mut Message,
        ) -> std::pin::Pin<Box<dyn Future<Output = Result<bool>> + Send + 'm>>
        + Send,
    ctx: &Context,
    channel: ChannelId,
    timeout: std::time::Duration,
) -> Result<()> {
    paginate(pager, ctx, channel, timeout).await
}

/// Same as `paginate_reply`, but for function inputs, especially anonymous functions.
pub async fn paginate_reply_fn(
    pager: impl for<'m> FnMut(
            u8,
            &'m Context,
            &'m mut Message,
        ) -> std::pin::Pin<Box<dyn Future<Output = Result<bool>> + Send + 'm>>
        + Send,
    ctx: &Context,
    reply_to: &Message,
    timeout: std::time::Duration,
) -> Result<()> {
    paginate_reply(pager, ctx, reply_to, timeout).await
}

// Handle the reaction and return a new page number.
pub async fn handle_pagination_reaction(
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
