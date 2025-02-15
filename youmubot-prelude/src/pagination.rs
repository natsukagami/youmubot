use crate::{editable_message, Context, Editable, OkPrint, Result};
use futures_util::{future::Future, StreamExt as _};
use serenity::{
    builder::CreateMessage,
    collector,
    model::{
        channel::{Message, Reaction, ReactionType},
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
    async fn render(&mut self, page: u8, ctx: &Context, m: &mut impl Editable) -> Result<bool>;

    /// Any setting-up before the rendering stage.
    async fn prerender(&mut self, _ctx: &Context, _m: &mut impl Editable) -> Result<()> {
        Ok(())
    }

    /// Cleans up after the pagination has timed out.
    async fn cleanup(&mut self, _ctx: &Context, _m: &mut impl Editable) -> () {}

    /// Handle the incoming reaction. Defaults to calling `handle_pagination_reaction`, but you can do some additional handling
    /// before handing the functionality over.
    ///
    /// Return the resulting current page, or `None` if the pagination should stop.
    async fn handle_reaction(
        &mut self,
        page: u8,
        ctx: &Context,
        message: &mut impl Editable,
        reaction: &Reaction,
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

    /// Add a page count to the pagination.
    fn with_page_count(self, page_count: usize) -> impl Paginate {
        WithPageCount {
            inner: self,
            page_count,
        }
    }
}

// pub fn paginate_from_fn(
//     pager: impl for<'m, M: Editable> FnMut(
//             u8,
//             &'m Context,
//             &'m mut M,
//         ) -> std::pin::Pin<
//             Box<dyn Future<Output = Result<bool>> + Send + 'm>,
//         > + Send,
// ) -> impl Paginate {
//     pager
// }

struct WithPageCount<Inner> {
    inner: Inner,
    page_count: usize,
}

#[async_trait::async_trait]
impl<Inner: Paginate> Paginate for WithPageCount<Inner> {
    async fn render(&mut self, page: u8, ctx: &Context, m: &mut impl Editable) -> Result<bool> {
        if page as usize >= self.page_count {
            return Ok(false);
        }
        self.inner.render(page, ctx, m).await
    }
    async fn prerender(&mut self, ctx: &Context, m: &mut impl Editable) -> Result<()> {
        self.inner.prerender(ctx, m).await
    }

    async fn handle_reaction(
        &mut self,
        page: u8,
        ctx: &Context,
        message: &mut impl Editable,
        reaction: &Reaction,
    ) -> Result<Option<u8>> {
        // handle normal reactions first, then fallback to the inner one
        let new_page = handle_pagination_reaction(page, self, ctx, message, reaction).await?;

        if new_page != page {
            Ok(Some(new_page))
        } else {
            self.inner
                .handle_reaction(page, ctx, message, reaction)
                .await
        }
    }

    fn len(&self) -> Option<usize> {
        Some(self.page_count)
    }

    fn is_empty(&self) -> Option<bool> {
        Some(self.page_count == 0)
    }

    async fn cleanup(&mut self, ctx: &Context, msg: &mut impl Editable) {
        self.inner.cleanup(ctx, msg).await;
    }
}

// #[async_trait::async_trait]
// impl<T> Paginate for T
// where
//     T: for<'m> FnMut(
//             u8,
//             &'m Context,
//             &'m mut Message,
//         ) -> std::pin::Pin<Box<dyn Future<Output = Result<bool>> + Send + 'm>>
//         + Send,
// {
//     async fn render(&mut self, page: u8, ctx: &Context, m: &mut impl Editable) -> Result<bool> {
//         self(page, ctx, m).await
//     }
// }

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
    let mut edit = editable_message(message, ctx.clone());
    paginate_with_first_message(pager, ctx, edit, timeout).await
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
        .send_message(
            &ctx,
            CreateMessage::new().content("Youmu is loading the first page..."),
        )
        .await?;
    let mut edit = editable_message(message, ctx.clone());
    paginate_with_first_message(pager, ctx, edit, timeout).await
}

/// Paginate with the first message already created.
pub async fn paginate_with_first_message(
    mut pager: impl Paginate,
    ctx: &Context,
    message: impl Editable,
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
    let reactions = {
        let mut rs = Vec::<Reaction>::with_capacity(4);
        // if large_count {
        //     // add >> and << buttons
        //     rs.push(message.react(&ctx, ReactionType::try_from(REWIND)?).await?);
        // }
        // rs.push(
        //     message
        //         .react(&ctx, ReactionType::try_from(ARROW_LEFT)?)
        //         .await?,
        // );
        // rs.push(
        //     message
        //         .react(&ctx, ReactionType::try_from(ARROW_RIGHT)?)
        //         .await?,
        // );
        // if large_count {
        //     // add >> and << buttons
        //     rs.push(
        //         message
        //             .react(&ctx, ReactionType::try_from(FAST_FORWARD)?)
        //             .await?,
        //     );
        // }
        rs
    };
    // Build a reaction collector
    let mut reaction_collector = {
        // message.await_reactions(ctx).removed(true).build();
        let message_id = message.id;
        let me = message.author.id;
        collector::collect(&ctx.shard, move |event| {
            match event {
                serenity::all::Event::ReactionAdd(r) => Some(r.reaction.clone()),
                serenity::all::Event::ReactionRemove(r) => Some(r.reaction.clone()),
                _ => None,
            }
            .filter(|r| r.message_id == message_id)
            .filter(|r| r.user_id.is_some_and(|id| id != me))
        })
    };
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

    pager.cleanup(ctx, &mut message).await;

    for reaction in reactions {
        if reaction.delete_all(&ctx).await.pls_ok().is_none() {
            // probably no permission to delete all reactions, fall back to delete my own.
            reaction.delete(&ctx).await.pls_ok();
        }
    }

    res
}

// Handle the reaction and return a new page number.
pub async fn handle_pagination_reaction(
    page: u8,
    pager: &mut impl Paginate,
    ctx: &Context,
    message: &mut impl Editable,
    reaction: &Reaction,
) -> Result<u8> {
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
