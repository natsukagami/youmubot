use poise::{CreateReply, ReplyHandle};
use serenity::{all::Message, builder::EditMessage};

use crate::*;

/// Represents a target where replying is possible and returns a message.
#[async_trait]
pub trait Replyable {
    type Resp: Updateable + Send;
    /// Reply to the context.
    async fn reply(
        &self,
        ctx: impl CacheHttp + Send,
        content: impl Into<String> + Send,
    ) -> Result<Self::Resp>;
}

#[async_trait]
impl Replyable for Message {
    type Resp = Message;
    async fn reply(
        &self,
        ctx: impl CacheHttp + Send,
        content: impl Into<String> + Send,
    ) -> Result<Self::Resp> {
        Ok(Message::reply(self, ctx, content).await?)
    }
}

#[async_trait]
impl<'c, T: Sync, E> Replyable for poise::Context<'c, T, E> {
    type Resp = (ReplyHandle<'c>, Self);
    async fn reply(
        &self,
        _ctx: impl CacheHttp + Send,
        content: impl Into<String> + Send,
    ) -> Result<Self::Resp> {
        let handle = poise::Context::reply(*self, content).await?;
        Ok((handle, *self))
    }
}

/// Represents a message representation that allows deletion and editing.
#[async_trait]
pub trait Updateable {
    async fn message(&self) -> Result<Message>;
    async fn edit(&mut self, ctx: impl CacheHttp + Send, content: CreateReply) -> Result<()>;
    async fn delete(&self, ctx: impl CacheHttp + Send) -> Result<()>;
}

#[async_trait]
impl Updateable for Message {
    async fn message(&self) -> Result<Message> {
        Ok(self.clone())
    }
    async fn edit(&mut self, ctx: impl CacheHttp + Send, content: CreateReply) -> Result<()> {
        let content = content.to_prefix_edit(EditMessage::new());
        Ok(Message::edit(self, ctx, content).await?)
    }
    async fn delete(&self, ctx: impl CacheHttp + Send) -> Result<()> {
        Ok(Message::delete(self, ctx).await?)
    }
}

#[async_trait]
impl<'a, T: Sync, E> Updateable for (poise::ReplyHandle<'a>, poise::Context<'a, T, E>) {
    async fn message(&self) -> Result<Message> {
        Ok(poise::ReplyHandle::message(&self.0).await?.into_owned())
    }
    async fn edit(&mut self, _ctx: impl CacheHttp, content: CreateReply) -> Result<()> {
        Ok(poise::ReplyHandle::edit(&self.0, self.1, content).await?)
    }
    async fn delete(&self, _ctx: impl CacheHttp) -> Result<()> {
        Ok(poise::ReplyHandle::delete(&self.0, self.1).await?)
    }
}
