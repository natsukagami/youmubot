use crossbeam_channel::{after, bounded, select, Sender};
use serenity::{framework::standard::CommandResult, model::channel::Reaction, prelude::*};
use std::sync::{Arc, Mutex};

/// Handles a reaction.
///
/// Every handler needs an expire time too.
pub trait ReactionHandler {
    /// Handle a reaction. This is fired on EVERY reaction.
    /// You do the filtering yourself.
    ///
    /// If `is_added` is false, the reaction was removed instead of added.
    fn handle_reaction(&mut self, reaction: &Reaction, is_added: bool) -> CommandResult;
}

impl<T> ReactionHandler for T
where
    T: FnMut(&Reaction, bool) -> CommandResult,
{
    fn handle_reaction(&mut self, reaction: &Reaction, is_added: bool) -> CommandResult {
        self(reaction, is_added)
    }
}

/// The store for a set of dynamic reaction handlers.
#[derive(Debug, Clone)]
pub struct ReactionWatcher {
    channels: Arc<Mutex<Vec<Sender<(Arc<Reaction>, bool)>>>>,
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
    /// If `is_added` is false, the reaction was removed.
    pub fn send(&self, r: Reaction, is_added: bool) {
        let r = Arc::new(r);
        self.channels
            .lock()
            .expect("Poisoned!")
            .retain(|e| e.send((r.clone(), is_added)).is_ok());
    }
    /// React! to a series of reaction
    ///
    /// The reactions stop after `duration` of idle.
    pub fn handle_reactions(
        &self,
        mut h: impl ReactionHandler,
        duration: std::time::Duration,
    ) -> CommandResult {
        let (send, reactions) = bounded(0);
        {
            self.channels.lock().expect("Poisoned!").push(send);
        }
        loop {
            let timeout = after(duration);
            let r = select! {
                recv(reactions) -> r => { let (r, is_added) = r.unwrap(); h.handle_reaction(&*r, is_added) },
                recv(timeout) -> _ => break,
            };
            if let Err(v) = r {
                dbg!(v);
            }
        }
        Ok(())
    }
    /// React! to a series of reaction
    ///
    /// The handler will stop after `duration` no matter what.
    pub fn handle_reactions_timed(
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
                recv(reactions) -> r => { let (r, is_added) = r.unwrap(); h.handle_reaction(&*r, is_added) },
                recv(timeout) -> _ => break,
            };
            if let Err(v) = r {
                dbg!(v);
            }
        }
        Ok(())
    }
}
