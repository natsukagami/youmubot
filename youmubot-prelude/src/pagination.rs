use crate::{
    replyable::{Replyable, Updateable},
    Context, OkPrint, Result,
};
use futures_util::{future::Future, StreamExt as _};
use poise::CreateReply;
use serenity::{
    builder::CreateMessage,
    collector,
    model::{
        channel::{Reaction, ReactionType},
        id::ChannelId,
    },
};
use std::convert::TryFrom;
use tokio::time as tokio_time;

const ARROW_RIGHT: &str = "➡️";
const ARROW_LEFT: &str = "⬅️";
const REWIND: &str = "⏪";
const FAST_FORWARD: &str = "⏩";

/// Represents a page update.
#[derive(Default)]
pub struct PageUpdate {
    pub message: Option<CreateReply>,
    pub page: Option<u8>,
    pub react: Vec<ReactionType>,
}

impl From<u8> for PageUpdate {
    fn from(value: u8) -> Self {
        PageUpdate {
            page: Some(value),
            ..Default::default()
        }
    }
}

impl From<CreateReply> for PageUpdate {
    fn from(value: CreateReply) -> Self {
        PageUpdate {
            message: Some(value),
            ..Default::default()
        }
    }
}

/// A trait that provides the implementation of a paginator.
#[async_trait::async_trait]
pub trait Paginate: Send + Sized {
    /// Render the given page.
    async fn render(&mut self, page: u8, ctx: &Context) -> Result<Option<CreateReply>>;

    /// Any setting-up before the rendering stage.
    async fn prerender(&mut self, _ctx: &Context) -> Result<PageUpdate> {
        Ok(PageUpdate::default())
    }

    /// Handle the incoming reaction. Defaults to calling `handle_pagination_reaction`, but you can do some additional handling
    /// before handing the functionality over.
    ///
    /// Return the resulting current page, or `None` if the pagination should stop.
    async fn handle_reaction(
        &mut self,
        page: u8,
        ctx: &Context,
        reaction: &Reaction,
    ) -> Result<PageUpdate> {
        handle_pagination_reaction(page, self, ctx, reaction).await
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
        ) -> std::pin::Pin<
            Box<dyn Future<Output = Result<Option<CreateReply>>> + Send + 'm>,
        > + Send,
{
    async fn render(&mut self, page: u8, ctx: &Context) -> Result<Option<CreateReply>> {
        self(page, ctx).await
    }
}

// Paginate! with a pager function, and replying to a message.
/// If awaited, will block until everything is done.
pub async fn paginate_reply(
    pager: impl Paginate,
    ctx: &Context,
    reply_to: impl Replyable,
    timeout: std::time::Duration,
) -> Result<()> {
    let update = reply_to
        .reply(&ctx, "Youmu is loading the first page...")
        .await?;
    paginate_with_first_message(pager, ctx, update, timeout).await
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
    paginate_with_first_message(pager, ctx, message, timeout).await
}

async fn paginate_with_first_message(
    mut pager: impl Paginate,
    ctx: &Context,
    mut update: impl Updateable,
    timeout: std::time::Duration,
) -> Result<()> {
    let message = update.message().await?;
    let prerender = pager.prerender(ctx).await?;
    if let Some(cr) = prerender.message {
        update.edit(ctx, cr).await?;
    }
    if let Some(cr) = pager.render(0, ctx).await? {
        update.edit(ctx, cr).await?;
    }
    // Just quit if there is only one page
    if pager.len().filter(|&v| v == 1).is_some() {
        return Ok(());
    }
    // React to the message
    let large_count = pager.len().filter(|&p| p > 10).is_some();
    let reactions = {
        let mut rs = Vec::<Reaction>::with_capacity(4 + prerender.react.len());
        if large_count {
            // add >> and << buttons
            rs.push(message.react(&ctx, ReactionType::try_from(REWIND)?).await?);
        }
        rs.push(
            message
                .react(&ctx, ReactionType::try_from(ARROW_LEFT)?)
                .await?,
        );
        rs.push(
            message
                .react(&ctx, ReactionType::try_from(ARROW_RIGHT)?)
                .await?,
        );
        if large_count {
            // add >> and << buttons
            rs.push(
                message
                    .react(&ctx, ReactionType::try_from(FAST_FORWARD)?)
                    .await?,
            );
        }
        for r in prerender.react.into_iter() {
            rs.push(message.react(&ctx, r).await?);
        }
        rs
    };
    // Build a reaction collector
    let mut reaction_collector = {
        // message.await_reactions(ctx).removed(true).build();
        let message_id = message.id;
        collector::collect(&ctx.shard, move |event| {
            match event {
                serenity::all::Event::ReactionAdd(r) => Some(r.reaction.clone()),
                serenity::all::Event::ReactionRemove(r) => Some(r.reaction.clone()),
                _ => None,
            }
            .filter(|r| r.message_id == message_id)
        })
    };
    let mut page = 0;

    // Loop the handler function.
    let res: Result<()> = loop {
        match tokio_time::timeout(timeout, reaction_collector.next()).await {
            Err(_) => break Ok(()),
            Ok(None) => break Ok(()),
            Ok(Some(reaction)) => {
                page = match pager.handle_reaction(page, ctx, &reaction).await {
                    Ok(pu) => {
                        if let Some(cr) = pu.message {
                            update.edit(ctx, cr).await?;
                        }
                        match pu.page {
                            Some(v) => v,
                            None => break Ok(()),
                        }
                    }
                    Err(e) => break Err(e),
                };
            }
        }
    };

    for reaction in reactions {
        if let None = reaction.delete_all(&ctx).await.pls_ok() {
            // probably no permission to delete all reactions, fall back to delete my own.
            reaction.delete(&ctx).await.pls_ok();
        }
    }

    res
}

/// Same as `paginate`, but for function inputs, especially anonymous functions.
pub async fn paginate_fn(
    pager: impl for<'m> FnMut(
            u8,
            &'m Context,
        ) -> std::pin::Pin<
            Box<dyn Future<Output = Result<Option<CreateReply>>> + Send + 'm>,
        > + Send,
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
        ) -> std::pin::Pin<
            Box<dyn Future<Output = Result<Option<CreateReply>>> + Send + 'm>,
        > + Send,
    ctx: &Context,
    reply_to: impl Replyable,
    timeout: std::time::Duration,
) -> Result<()> {
    paginate_reply(pager, ctx, reply_to, timeout).await
}

// Handle the reaction and return a new page number.
pub async fn handle_pagination_reaction(
    page: u8,
    pager: &mut impl Paginate,
    ctx: &Context,
    reaction: &Reaction,
) -> Result<PageUpdate> {
    let pages = pager.len();
    let fast = pages.map(|v| v / 10).unwrap_or(5).max(5) as u8;
    match &reaction.emoji {
        ReactionType::Unicode(ref s) => {
            let new_page = match s.as_str() {
                ARROW_LEFT | REWIND if page == 0 => return Ok(page.into()),
                ARROW_LEFT => page - 1,
                REWIND => {
                    if page < fast {
                        0
                    } else {
                        page - fast
                    }
                }
                ARROW_RIGHT if pages.filter(|&pages| page as usize + 1 >= pages).is_some() => {
                    return Ok(page.into())
                }
                ARROW_RIGHT => page + 1,
                FAST_FORWARD => (pages.unwrap() as u8 - 1).min(page + fast),
                _ => return Ok(page.into()),
            };
            let reply = pager.render(new_page, ctx).await?;
            Ok(reply
                .map(|cr| PageUpdate {
                    message: Some(cr),
                    page: Some(page),
                    ..Default::default()
                })
                .unwrap_or_else(|| page.into()))
            // Ok(if pager.render(new_page, ctx, message).await? {
            //     new_page
            // } else {
            //     page
            // })
        }
        _ => Ok(page.into()),
    }
}
