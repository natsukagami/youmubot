use crate::{Context, OkPrint, Result};
use futures_util::future::Future;
use serenity::{
    all::{
        CreateActionRow, CreateButton, CreateInteractionResponse, EditMessage, Interaction,
        MessageId,
    },
    builder::CreateMessage,
    model::{
        channel::{Message, ReactionType},
        id::ChannelId,
    },
    prelude::TypeMapKey,
};
use std::{convert::TryFrom, sync::Arc};
use tokio::time as tokio_time;

const ARROW_RIGHT: &str = "➡️";
const ARROW_LEFT: &str = "⬅️";
const REWIND: &str = "⏪";
const FAST_FORWARD: &str = "⏩";

const NEXT: &str = "youmubot_pagination_next";
const PREV: &str = "youmubot_pagination_prev";
const FAST_NEXT: &str = "youmubot_pagination_fast_next";
const FAST_PREV: &str = "youmubot_pagination_fast_prev";

/// A trait that provides the implementation of a paginator.
#[async_trait::async_trait]
pub trait Paginate: Send + Sized {
    /// Render the given page.
    /// Remember to add the [[interaction_buttons]] as an action row!
    async fn render(
        &mut self,
        page: u8,
        ctx: &Context,
        m: &Message,
        btns: Vec<CreateActionRow>,
    ) -> Result<Option<EditMessage>>;

    // /// The [[CreateActionRow]] for pagination.
    // fn pagination_row(&self) -> CreateActionRow {
    // }

    /// A list of buttons to create that would interact with pagination logic.
    fn interaction_buttons(&self) -> Vec<CreateButton> {
        default_buttons(self)
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
        reaction: &str,
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

pub async fn do_render(
    p: &mut impl Paginate,
    page: u8,
    ctx: &Context,
    m: &mut Message,
) -> Result<bool> {
    let btns = vec![CreateActionRow::Buttons(p.interaction_buttons())];
    do_render_with_btns(p, page, ctx, m, btns).await
}

async fn do_render_with_btns(
    p: &mut impl Paginate,
    page: u8,
    ctx: &Context,
    m: &mut Message,
    btns: Vec<CreateActionRow>,
) -> Result<bool> {
    if let Some(edit) = p.render(page, ctx, m, btns).await? {
        m.edit(ctx, edit).await?;
        Ok(true)
    } else {
        Ok(false)
    }
}

pub fn paginate_from_fn(
    pager: impl for<'m> FnMut(
            u8,
            &'m Context,
            &'m Message,
            Vec<CreateActionRow>,
        ) -> std::pin::Pin<
            Box<dyn Future<Output = Result<Option<EditMessage>>> + Send + 'm>,
        > + Send,
) -> impl Paginate {
    pager
}

pub fn default_buttons(p: &impl Paginate) -> Vec<CreateButton> {
    let mut btns = vec![
        CreateButton::new(PREV).emoji(ReactionType::try_from(ARROW_LEFT).unwrap()),
        CreateButton::new(NEXT).emoji(ReactionType::try_from(ARROW_RIGHT).unwrap()),
    ];
    if p.len().is_some_and(|v| v > 5) {
        btns.insert(
            0,
            CreateButton::new(FAST_PREV).emoji(ReactionType::try_from(REWIND).unwrap()),
        );
        btns.push(CreateButton::new(FAST_NEXT).emoji(ReactionType::try_from(FAST_FORWARD).unwrap()))
    }
    btns
}

struct WithPageCount<Inner> {
    inner: Inner,
    page_count: usize,
}

#[async_trait::async_trait]
impl<Inner: Paginate> Paginate for WithPageCount<Inner> {
    async fn render(
        &mut self,
        page: u8,
        ctx: &Context,
        m: &Message,
        btns: Vec<CreateActionRow>,
    ) -> Result<Option<EditMessage>> {
        if page as usize >= self.page_count {
            return Ok(None);
        }
        self.inner.render(page, ctx, m, btns).await
    }

    async fn handle_reaction(
        &mut self,
        page: u8,
        ctx: &Context,
        message: &mut Message,
        reaction: &str,
    ) -> Result<Option<u8>> {
        self.inner
            .handle_reaction(page, ctx, message, reaction)
            .await
    }

    fn len(&self) -> Option<usize> {
        Some(self.page_count)
    }

    fn is_empty(&self) -> Option<bool> {
        Some(self.page_count == 0)
    }
}

#[async_trait::async_trait]
impl<T> Paginate for T
where
    T: for<'m> FnMut(
            u8,
            &'m Context,
            &'m Message,
            Vec<CreateActionRow>,
        ) -> std::pin::Pin<
            Box<dyn Future<Output = Result<Option<EditMessage>>> + Send + 'm>,
        > + Send,
{
    async fn render(
        &mut self,
        page: u8,
        ctx: &Context,
        m: &Message,
        btns: Vec<CreateActionRow>,
    ) -> Result<Option<EditMessage>> {
        self(page, ctx, m, btns).await
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
        .send_message(
            &ctx,
            CreateMessage::new().content("Youmu is loading the first page..."),
        )
        .await?;
    paginate_with_first_message(pager, ctx, message, timeout).await
}

/// Paginate with the first message already created.
pub async fn paginate_with_first_message(
    mut pager: impl Paginate,
    ctx: &Context,
    mut message: Message,
    timeout: std::time::Duration,
) -> Result<()> {
    let (send, recv) = flume::unbounded::<String>();
    Paginator::push(ctx, &message, send).await?;

    do_render(&mut pager, 0, ctx, &mut message).await?;
    // Just quit if there is only one page
    if pager.len().filter(|&v| v == 1).is_some() {
        return Ok(());
    }
    let mut page = 0;

    // Loop the handler function.
    let res: Result<()> = loop {
        match tokio_time::timeout(timeout, recv.clone().into_recv_async()).await {
            Err(_) => break Ok(()),
            Ok(Err(_)) => break Ok(()),
            Ok(Ok(reaction)) => {
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

    // Render one last time with no buttons
    do_render_with_btns(&mut pager, page, ctx, &mut message, vec![])
        .await
        .pls_ok();
    Paginator::pop(ctx, &message).await?;

    res
}

// Handle the reaction and return a new page number.
pub async fn handle_pagination_reaction(
    page: u8,
    pager: &mut impl Paginate,
    ctx: &Context,
    message: &mut Message,
    reaction: &str,
) -> Result<u8> {
    let pages = pager.len();
    let fast = pages.map(|v| v / 10).unwrap_or(5).max(5) as u8;
    let new_page = match reaction {
        PREV | FAST_PREV if page == 0 => return Ok(page),
        PREV => page - 1,
        FAST_PREV => {
            if page < fast {
                0
            } else {
                page - fast
            }
        }
        NEXT if pages.filter(|&pages| page as usize + 1 >= pages).is_some() => return Ok(page),
        NEXT => page + 1,
        FAST_NEXT => (pages.unwrap() as u8 - 1).min(page + fast),
        _ => return Ok(page),
    };
    Ok(if do_render(pager, new_page, ctx, message).await? {
        new_page
    } else {
        page
    })
}

#[derive(Debug, Clone)]
/// Handles distributing pagination interaction to the handlers.
pub struct Paginator {
    pub(crate) channels: Arc<dashmap::DashMap<MessageId, flume::Sender<String>>>,
}

impl Paginator {
    pub fn new() -> Self {
        Self {
            channels: Arc::new(dashmap::DashMap::new()),
        }
    }
    async fn push(ctx: &Context, msg: &Message, channel: flume::Sender<String>) -> Result<()> {
        ctx.data
            .write()
            .await
            .get_mut::<Paginator>()
            .unwrap()
            .channels
            .insert(msg.id, channel);
        Ok(())
    }

    async fn pop(ctx: &Context, msg: &Message) -> Result<()> {
        ctx.data
            .write()
            .await
            .get_mut::<Paginator>()
            .unwrap()
            .channels
            .remove(&msg.id);
        Ok(())
    }
}

impl TypeMapKey for Paginator {
    type Value = Paginator;
}

#[async_trait::async_trait]
impl crate::hook::InteractionHook for Paginator {
    async fn call(&self, ctx: &Context, interaction: &Interaction) -> Result<()> {
        match interaction {
            Interaction::Component(component_interaction) => {
                if let Some(ch) = self.channels.get(&component_interaction.message.id) {
                    component_interaction
                        .create_response(ctx, CreateInteractionResponse::Acknowledge)
                        .await?;
                    ch.send_async(component_interaction.data.custom_id.clone())
                        .await
                        .ok();
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
}
