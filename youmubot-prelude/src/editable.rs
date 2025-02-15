use poise::{CreateReply, ReplyHandle};
use serenity::all::Message;

use crate::CmdContext;
use core::future::Future;

/// Represents an editable message context.
pub trait Editable: Send {
    /// Edits the underlying message.
    fn edit_msg(&mut self, reply: CreateReply) -> impl Future<Output = anyhow::Result<()>> + Send;
}

struct ReplyHandleEdit<'a, 'env, Env>(ReplyHandle<'a>, CmdContext<'env, Env>);

impl<'a, 'b, Env: Send + Sync> Editable for ReplyHandleEdit<'a, 'b, Env> {
    async fn edit_msg(&mut self, reply: CreateReply) -> anyhow::Result<()> {
        Ok(self.0.edit(self.1.clone(), reply).await?)
    }
}

/// Returns an [`Editable`] from a [`ReplyHandle`].
pub fn editable_reply_handle<'a, 'b, Env: Send + Sync>(
    reply: ReplyHandle<'a>,
    ctx: CmdContext<'b, Env>,
) -> impl Editable + use<'a, 'b, Env> {
    ReplyHandleEdit(reply, ctx)
}

struct MsgEdit(Message, serenity::all::Context);

/// Returns an [`Editable`] from a [`Message`].
pub fn editable_message(msg: Message, ctx: serenity::all::Context) -> impl Editable {
    MsgEdit(msg, ctx)
}

impl Editable for MsgEdit {
    async fn edit_msg(&mut self, reply: CreateReply) -> anyhow::Result<()> {
        self.0
            .edit(&self.1, {
                // Clear builder so that adding embeds or attachments won't add on top of
                // the pre-edit items but replace them (which is apparently the more
                // intuitive behavior). Notably, setting the builder to default doesn't
                // mean the entire message is reset to empty: Discord only updates parts
                // of the message that have had a modification specified
                reply.to_prefix_edit(serenity::all::EditMessage::new())
            })
            .await?;
        Ok(())
    }
}
