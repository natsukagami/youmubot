use crossbeam_channel::{after, bounded, select, Sender};
use serenity::{framework::standard::CommandResult, model::channel::Reaction, prelude::*};
use std::sync::{Arc, Mutex};

/// Handles a reaction.
///
/// Every handler needs an expire time too.
pub trait ReactionHandler {
    /// Handle a reaction. This is fired on EVERY reaction.
    /// You do the filtering yourself.
    fn handle_reaction(&mut self, reaction: &Reaction) -> CommandResult;
}

impl<T> ReactionHandler for T
where
    T: FnMut(&Reaction) -> CommandResult,
{
    fn handle_reaction(&mut self, reaction: &Reaction) -> CommandResult {
        self(reaction)
    }
}

/// The store for a set of dynamic reaction handlers.
#[derive(Debug, Clone)]
pub struct ReactionWatcher {
    channels: Arc<Mutex<Vec<Sender<Arc<Reaction>>>>>,
}

impl TypeMapKey for ReactionWatcher {
    type Value = ReactionWatcher;
}

impl ReactionWatcher {
    /// Create a new ReactionWatcher.
    pub fn new() -> Self {
        Self {
            channels: Arc::new(Mutex::new(vec![])),
        }
    }
    /// Send a reaction.
    pub fn send(&self, r: Reaction) {
        let r = Arc::new(r);
        self.channels
            .lock()
            .expect("Poisoned!")
            .retain(|e| e.send(r.clone()).is_ok());
    }
    /// React! to a series of reaction
    ///
    /// The reactions stop after `duration`.
    pub fn handle_reactions(
        &self,
        mut h: impl ReactionHandler,
        duration: std::time::Duration,
    ) -> CommandResult {
        let (send, reactions) = bounded(0);
        {
            self.channels.lock().expect("Poisoned!").push(send);
        }
        let timeout = after(duration);
        loop {
            let r = select! {
                recv(reactions) -> r => h.handle_reaction(&*r.unwrap()),
                recv(timeout) -> _ => break,
            };
            if let Err(v) = r {
                return Err(v);
            }
        }
        Ok(())
    }
}
