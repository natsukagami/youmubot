use std::{future::Future, pin::Pin};

use serenity::{client::FullEvent, framework::Framework, model::channel::Message};
use youmubot_prelude::*;

/// A Framework to compose other frameworks.
pub(crate) struct ComposedFramework {
    frameworks: Box<[Box<dyn Framework>]>,
}

impl ComposedFramework {
    /// Create a new composed framework.
    pub fn new(frameworks: Vec<Box<dyn Framework>>) -> Self {
        Self {
            frameworks: frameworks.into_boxed_slice(),
        }
    }
}

#[async_trait]
impl Framework for ComposedFramework {
    async fn dispatch(&self, ctx: Context, msg: FullEvent) -> () {
        if !self.frameworks.is_empty() {
            self.dispatch_loop(self.frameworks.len() - 1, ctx, msg)
                .await
        }
    }
}
impl ComposedFramework {
    /// Dispatch to all inner frameworks in a loop. Returns a `Pin<Box<Future>>` because rust.
    fn dispatch_loop<'a>(
        &'a self,
        index: usize,
        ctx: Context,
        msg: FullEvent,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            if index == 0 {
                self.frameworks[index].dispatch(ctx, msg).await
            } else {
                self.frameworks[index]
                    .dispatch(ctx.clone(), msg.clone())
                    .await;
                self.dispatch_loop(index - 1, ctx, msg).await
            }
        })
    }
}
