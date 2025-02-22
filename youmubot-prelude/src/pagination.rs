use crate::{CmdContext, Context, OkPrint, Result};
use futures_util::future::Future;
use poise::{CreateReply, ReplyHandle};
use serenity::{
    all::{
        ComponentInteraction, CreateActionRow, CreateButton, EditInteractionResponse, EditMessage,
    },
    builder::CreateMessage,
    model::{channel::Message, id::ChannelId},
};
use tokio::time as tokio_time;

// const ARROW_RIGHT: &str = "➡️";
// const ARROW_LEFT: &str = "⬅️";
// const REWIND: &str = "⏪";
// const FAST_FORWARD: &str = "⏩";

const NEXT: &str = "youmubot_pagination_next";
const PREV: &str = "youmubot_pagination_prev";
const FAST_NEXT: &str = "youmubot_pagination_fast_next";
const FAST_PREV: &str = "youmubot_pagination_fast_prev";

pub trait CanEdit: Send {
    fn get_message(&self) -> impl Future<Output = Result<Message>> + Send;
    fn apply_edit(&mut self, edit: CreateReply) -> impl Future<Output = Result<()>> + Send;
}

impl<'a> CanEdit for (Message, &'a Context) {
    async fn get_message(&self) -> Result<Message> {
        Ok(self.0.clone())
    }

    async fn apply_edit(&mut self, edit: CreateReply) -> Result<()> {
        self.0
            .edit(&self.1, edit.to_prefix_edit(EditMessage::new()))
            .await?;
        Ok(())
    }
}

impl<'a, 'b> CanEdit for (&'a ComponentInteraction, &'b Context) {
    async fn get_message(&self) -> Result<Message> {
        Ok(self.0.get_response(&self.1.http).await?)
    }

    async fn apply_edit(&mut self, edit: CreateReply) -> Result<()> {
        self.0
            .edit_response(
                &self.1,
                edit.to_slash_initial_response_edit(EditInteractionResponse::new()),
            )
            .await?;
        Ok(())
    }
}

impl<'a, 'e, Env: Send + Sync> CanEdit for (ReplyHandle<'a>, CmdContext<'e, Env>) {
    async fn get_message(&self) -> Result<Message> {
        Ok(self.0.message().await?.into_owned())
    }

    async fn apply_edit(&mut self, edit: CreateReply) -> Result<()> {
        self.0.edit(self.1, edit).await?;
        Ok(())
    }
}

/// A trait that provides the implementation of a paginator.
#[async_trait::async_trait]
pub trait Paginate: Send + Sized {
    /// Render the given page.
    /// Remember to add the [[interaction_buttons]] as an action row!
    async fn render(&mut self, page: u8, btns: Vec<CreateActionRow>)
        -> Result<Option<CreateReply>>;

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
        _ctx: &Context,
        message: &mut impl CanEdit,
        reaction: &str,
    ) -> Result<Option<u8>> {
        handle_pagination_reaction(page, self, message, reaction)
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

pub async fn do_render(p: &mut impl Paginate, page: u8, m: &mut impl CanEdit) -> Result<bool> {
    let btns = vec![CreateActionRow::Buttons(p.interaction_buttons())];
    do_render_with_btns(p, page, m, btns).await
}

async fn do_render_with_btns(
    p: &mut impl Paginate,
    page: u8,
    m: &mut impl CanEdit,
    btns: Vec<CreateActionRow>,
) -> Result<bool> {
    if let Some(edit) = p.render(page, btns).await? {
        m.apply_edit(edit).await?;
        Ok(true)
    } else {
        Ok(false)
    }
}

pub fn paginate_from_fn(
    pager: impl FnMut(
            u8,
            Vec<CreateActionRow>,
        )
            -> std::pin::Pin<Box<dyn Future<Output = Result<Option<CreateReply>>> + Send>>
        + Send,
) -> impl Paginate {
    pager
}

pub fn default_buttons(p: &impl Paginate) -> Vec<CreateButton> {
    let mut btns = vec![
        CreateButton::new(PREV).label("<"),
        CreateButton::new(NEXT).label(">"),
    ];
    if p.len().is_some_and(|v| v > 5) {
        btns.insert(0, CreateButton::new(FAST_PREV).label("<<"));
        btns.push(CreateButton::new(FAST_NEXT).label(">>"))
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
        btns: Vec<CreateActionRow>,
    ) -> Result<Option<CreateReply>> {
        if page as usize >= self.page_count {
            return Ok(None);
        }
        self.inner.render(page, btns).await
    }

    async fn handle_reaction(
        &mut self,
        page: u8,
        ctx: &Context,
        message: &mut impl CanEdit,
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
    T: FnMut(
            u8,
            Vec<CreateActionRow>,
        )
            -> std::pin::Pin<Box<dyn Future<Output = Result<Option<CreateReply>>> + Send>>
        + Send,
{
    async fn render(
        &mut self,
        page: u8,
        btns: Vec<CreateActionRow>,
    ) -> Result<Option<CreateReply>> {
        self(page, btns).await
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
    let message = (message, ctx);
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
    let message = (message, ctx);
    paginate_with_first_message(pager, ctx, message, timeout).await
}

/// Paginate with the first message already created.
pub async fn paginate_with_first_message(
    mut pager: impl Paginate,
    ctx: &Context,
    mut message: impl CanEdit,
    timeout: std::time::Duration,
) -> Result<()> {
    let msg_id = message.get_message().await?.id;
    let recv = crate::InteractionCollector::create(ctx, msg_id).await?;

    do_render(&mut pager, 0, &mut message).await?;
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
    do_render_with_btns(&mut pager, page, &mut message, vec![])
        .await
        .pls_ok();

    res
}

// Handle the reaction and return a new page number.
pub async fn handle_pagination_reaction(
    page: u8,
    pager: &mut impl Paginate,
    message: &mut impl CanEdit,
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
    Ok(if do_render(pager, new_page, message).await? {
        new_page
    } else {
        page
    })
}
