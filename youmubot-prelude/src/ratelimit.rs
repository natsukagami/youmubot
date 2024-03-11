use std::ops::Deref;
/// Provides a simple ratelimit lock (that only works in tokio)
// use tokio::time::
use std::time::Duration;

use flume::{bounded as channel, Receiver, Sender};

use crate::Result;

/// Holds the underlying `T` in a rate-limited way.
#[derive(Debug, Clone)]
pub struct Ratelimit<T> {
    inner: T,
    recv: Receiver<()>,
    send: Sender<()>,

    wait_time: Duration,
}

struct RatelimitGuard<'a, T> {
    inner: &'a T,
    send: &'a Sender<()>,
    wait_time: &'a Duration,
}

impl<T> Ratelimit<T> {
    /// Create a new ratelimit with at most `count` uses in `wait_time`.
    pub fn new(inner: T, count: usize, wait_time: Duration) -> Self {
        let (send, recv) = channel(count);
        (0..count).for_each(|_| {
            send.send(()).ok();
        });
        Self {
            inner,
            recv,
            send,
            wait_time,
        }
    }

    /// Borrow the inner `T`. You can only hol this reference `count` times in `wait_time`.
    /// The clock counts from the moment the ref is dropped.
    pub async fn borrow(&self) -> Result<impl Deref<Target = T> + '_> {
        self.recv.recv_async().await?;
        Ok(RatelimitGuard {
            inner: &self.inner,
            send: &self.send,
            wait_time: &self.wait_time,
        })
    }
}

impl<'a, T> Deref for RatelimitGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.inner
    }
}

impl<'a, T> Drop for RatelimitGuard<'a, T> {
    fn drop(&mut self) {
        let send = self.send.clone();
        let wait_time = *self.wait_time;
        tokio::spawn(async move {
            tokio::time::sleep(wait_time).await;
            send.send_async(()).await.ok();
        });
    }
}
