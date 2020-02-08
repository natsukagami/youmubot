use crate::{Context, ReactionHandler, ReactionWatcher};
use serenity::{
    builder::EditMessage,
    framework::standard::{CommandError, CommandResult},
    model::{
        channel::{Message, Reaction, ReactionType},
        id::ChannelId,
    },
};

const ARROW_RIGHT: &'static str = "➡️";
const ARROW_LEFT: &'static str = "⬅️";

impl ReactionWatcher {
    /// Start a pagination.
    ///
    /// Takes a copy of Context (which you can `clone`), a pager (see "Pagination") and a target channel id.
    /// Pagination will handle all events on adding/removing an "arrow" emoji (⬅️ and ➡️).
    /// This is a blocking call - it will block the thread until duration is over.
    pub fn paginate<T: Pagination>(
        &self,
        ctx: Context,
        channel: ChannelId,
        pager: T,
        duration: std::time::Duration,
    ) -> CommandResult {
        let handler = PaginationHandler::new(pager, ctx, channel)?;
        self.handle_reactions(handler, duration)
    }

    /// A version of `paginate` that compiles for closures.
    ///
    /// A workaround until https://github.com/rust-lang/rust/issues/36582 is solved.
    pub fn paginate_fn<T>(
        &self,
        ctx: Context,
        channel: ChannelId,
        pager: T,
        duration: std::time::Duration,
    ) -> CommandResult
    where
        T: for<'a> Fn(u8, &'a mut EditMessage) -> (&'a mut EditMessage, CommandResult),
    {
        self.paginate(ctx, channel, pager, duration)
    }
}

/// Pagination allows the bot to display content in multiple pages.
///
/// You need to implement the "render_page" function, which takes a dummy content and
/// embed assigning function.
/// Pagination is automatically implemented for functions with the same signature as `render_page`.
///
/// Pages start at 0.
pub trait Pagination {
    /// Render a page.
    ///
    /// This would either create or edit a message, but you should not be worry about it.
    fn render_page<'a>(
        &self,
        page: u8,
        target: &'a mut EditMessage,
    ) -> (&'a mut EditMessage, CommandResult);
}

impl<T> Pagination for T
where
    T: for<'a> Fn(u8, &'a mut EditMessage) -> (&'a mut EditMessage, CommandResult),
{
    fn render_page<'a>(
        &self,
        page: u8,
        target: &'a mut EditMessage,
    ) -> (&'a mut EditMessage, CommandResult) {
        self(page, target)
    }
}

struct PaginationHandler<T: Pagination> {
    pager: T,
    message: Message,
    page: u8,
    ctx: Context,
}

impl<T: Pagination> PaginationHandler<T> {
    pub fn new(pager: T, mut ctx: Context, channel: ChannelId) -> Result<Self, CommandError> {
        let message = channel.send_message(&mut ctx, |e| {
            e.content("Youmu is loading the first page...")
        })?;
        // React to the message
        message.react(&mut ctx, ARROW_LEFT)?;
        message.react(&mut ctx, ARROW_RIGHT)?;
        let mut p = Self {
            pager,
            message: message.clone(),
            page: 0,
            ctx,
        };
        p.call_pager()?;
        Ok(p)
    }
}

impl<T: Pagination> PaginationHandler<T> {
    /// Call the pager, log the error (if any).
    fn call_pager(&mut self) -> CommandResult {
        let mut res: CommandResult = Ok(());
        let mut msg = self.message.clone();
        msg.edit(&self.ctx, |e| {
            let (e, r) = self.pager.render_page(self.page, e);
            res = r;
            e
        })?;
        self.message = msg;
        res
    }
}

impl<T: Pagination> ReactionHandler for PaginationHandler<T> {
    fn handle_reaction(&mut self, reaction: &Reaction, _is_add: bool) -> CommandResult {
        if reaction.message_id != self.message.id {
            return Ok(());
        }
        match &reaction.emoji {
            ReactionType::Unicode(ref s) => match s.as_str() {
                ARROW_LEFT if self.page == 0 => return Ok(()),
                ARROW_LEFT => {
                    self.page -= 1;
                    if let Err(e) = self.call_pager() {
                        self.page += 1;
                        return Err(e);
                    }
                }
                ARROW_RIGHT => {
                    self.page += 1;
                    if let Err(e) = self.call_pager() {
                        self.page -= 1;
                        return Err(e);
                    }
                }
                _ => (),
            },
            _ => (),
        }
        Ok(())
    }
}
