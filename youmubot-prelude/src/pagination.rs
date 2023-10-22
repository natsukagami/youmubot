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
const REWIND: &str = "⏪";
const FAST_FORWARD: &str = "⏩";

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

    /// Return the number of pages, if it is known in advance.
    /// If this is given, bounds-check will be done outside of `prerender` / `render`.
    fn len(&self) -> Option<usize> {
        None
    }

    fn is_empty(&self) -> Option<bool> {
        self.len().map(|v| v == 0)
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
    pager.prerender(ctx, &mut message).await?;
    pager.render(0, ctx, &mut message).await?;
    // Just quit if there is only one page
    if pager.len().filter(|&v| v == 1).is_some() {
        return Ok(());
    }
    // React to the message
    let large_count = pager.len().filter(|&p| p > 10).is_some();
    if large_count {
        // add >> and << buttons
        message.react(&ctx, ReactionType::try_from(REWIND)?).await?;
    }
    message
        .react(&ctx, ReactionType::try_from(ARROW_LEFT)?)
        .await?;
    message
        .react(&ctx, ReactionType::try_from(ARROW_RIGHT)?)
        .await?;
    if large_count {
        // add >> and << buttons
        message
            .react(&ctx, ReactionType::try_from(FAST_FORWARD)?)
            .await?;
    }
    // Build a reaction collector
    let mut reaction_collector = message.await_reactions(ctx).removed(true).build();
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
    let pages = pager.len();
    let fast = pages.map(|v| v / 10).unwrap_or(5).max(5) as u8;
    match &reaction.emoji {
        ReactionType::Unicode(ref s) => {
            let new_page = match s.as_str() {
                ARROW_LEFT | REWIND if page == 0 => return Ok(page),
                ARROW_LEFT => page - 1,
                REWIND => {
                    if page < fast {
                        0
                    } else {
                        page - fast
                    }
                }
                ARROW_RIGHT if pages.filter(|&pages| page as usize + 1 >= pages).is_some() => {
                    return Ok(page)
                }
                ARROW_RIGHT => page + 1,
                FAST_FORWARD => (pages.unwrap() as u8 - 1).min(page + fast),
                _ => return Ok(page),
            };
            Ok(if pager.render(new_page, ctx, message).await? {
                new_page
            } else {
                page
            })
        }
        _ => Ok(page),
    }
}
