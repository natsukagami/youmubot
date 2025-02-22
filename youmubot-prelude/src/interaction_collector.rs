use crate::{Context, Result};
use serenity::{
    all::{CreateInteractionResponse, Interaction, MessageId},
    prelude::TypeMapKey,
};
use std::{ops::Deref, sync::Arc};

#[derive(Debug, Clone)]
/// Handles distributing interaction to the handlers.
pub struct InteractionCollector {
    pub(crate) channels: Arc<dashmap::DashMap<MessageId, flume::Sender<String>>>,
}

/// Wraps the interfaction receiver channel, automatically cleaning up upon drop.
#[derive(Debug)]
struct InteractionCollectorGuard {
    msg_id: MessageId,
    ch: flume::Receiver<String>,
    collector: InteractionCollector,
}

impl Deref for InteractionCollectorGuard {
    type Target = flume::Receiver<String>;

    fn deref(&self) -> &Self::Target {
        &self.ch
    }
}

impl Drop for InteractionCollectorGuard {
    fn drop(&mut self) {
        self.collector.channels.remove(&self.msg_id);
    }
}

impl InteractionCollector {
    pub fn new() -> Self {
        Self {
            channels: Arc::new(dashmap::DashMap::new()),
        }
    }
    /// Create a new collector, returning a receiver.
    pub fn create_collector(&self, msg: MessageId) -> impl Deref<Target = flume::Receiver<String>> {
        let (send, recv) = flume::unbounded();
        self.channels.insert(msg.clone(), send);
        InteractionCollectorGuard {
            msg_id: msg,
            ch: recv,
            collector: self.clone(),
        }
    }

    /// Create a new collector, returning a receiver.
    pub(crate) async fn create(
        ctx: &Context,
        msg: MessageId,
    ) -> Result<impl Deref<Target = flume::Receiver<String>>> {
        Ok(ctx
            .data
            .read()
            .await
            .get::<InteractionCollector>()
            .unwrap()
            .create_collector(msg))
    }
}

impl TypeMapKey for InteractionCollector {
    type Value = InteractionCollector;
}

#[async_trait::async_trait]
impl crate::hook::InteractionHook for InteractionCollector {
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
