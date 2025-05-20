use std::{fmt::Display, future::Future, ops::Range};

use youmubot_prelude::*;

use crate::OsuClient;

pub const MAX_SCORE_PER_PAGE: usize = 1000;

/// Fetch scores given an offset.
/// Implemented for score requests.
pub trait Fetch: Send {
    type Item: Send + Sync + 'static;
    /// Scores per page.
    const ITEMS_PER_PAGE: usize = MAX_SCORE_PER_PAGE;
    /// Fetch items given an offset.
    fn fetch(
        &self,
        client: &crate::OsuClient,
        offset: usize,
    ) -> impl Future<Output = Result<Vec<Self::Item>>> + Send;

    /// Create a buffer from the given Fetch implementation.
    fn make_buffer(
        self,
        client: crate::OsuClient,
    ) -> impl Future<Output = Result<impl LazyBuffer<Self::Item>>> + Send
    where
        Self: Sized,
    {
        Fetcher::new(client, self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Size {
    /// There might be more
    AtLeast(usize),
    /// All
    Total(usize),
}

impl Display for Size {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.count())?;
        if !self.is_total() {
            write!(f, "+")?;
        }
        Ok(())
    }
}

impl Size {
    pub fn count(&self) -> usize {
        match self {
            Size::AtLeast(cnt) => *cnt,
            Size::Total(cnt) => *cnt,
        }
    }

    pub fn is_total(&self) -> bool {
        match self {
            Size::AtLeast(_) => false,
            Size::Total(_) => true,
        }
    }

    pub fn as_pages(self, per_page: usize) -> Size {
        match self {
            Size::AtLeast(a) => Size::AtLeast(a.div_ceil(per_page)),
            Size::Total(a) => Size::Total(a.div_ceil(per_page)),
        }
    }
}

/// A scores stream.
pub trait LazyBuffer<T: Send + Sync + 'static>: Send {
    /// Total length of the pages.
    fn length_fetched(&self) -> Size;

    /// Whether the scores set is empty.
    fn is_empty(&self) -> bool;

    /// Get the index-th score.
    fn get(&mut self, index: usize) -> impl Future<Output = Result<Option<&T>>> + Send;

    /// Get all scores.
    fn get_all(self) -> impl Future<Output = Result<Vec<T>>> + Send;

    /// Get the scores between the given range.
    fn get_range(&mut self, range: Range<usize>) -> impl Future<Output = Result<&[T]>> + Send;

    /// Find a score that matches the predicate `f`.
    fn find<F: FnMut(&T) -> bool + Send>(
        &mut self,
        f: F,
    ) -> impl Future<Output = Result<Option<&T>>> + Send;
}

impl<T: Send + Sync + 'static> LazyBuffer<T> for Vec<T> {
    fn length_fetched(&self) -> Size {
        Size::Total(self.len())
    }

    fn is_empty(&self) -> bool {
        self.is_empty()
    }

    fn get(&mut self, index: usize) -> impl Future<Output = Result<Option<&T>>> + Send {
        future::ok(self[..].get(index))
    }

    fn get_all(self) -> impl Future<Output = Result<Vec<T>>> + Send {
        future::ok(self)
    }

    fn get_range(&mut self, range: Range<usize>) -> impl Future<Output = Result<&[T]>> + Send {
        future::ok(&self[fit_range_to_len(self.len(), range)])
    }

    async fn find<F: FnMut(&T) -> bool + Send>(&mut self, mut f: F) -> Result<Option<&T>> {
        Ok(self.iter().find(|v| f(v)))
    }
}

#[inline]
fn fit_range_to_len(len: usize, range: Range<usize>) -> Range<usize> {
    range.start.min(len)..range.end.min(len)
}

/// A scores stream with a fetcher.
struct Fetcher<T: Fetch> {
    fetcher: T,
    client: OsuClient,
    items: Vec<T::Item>,
    more_exists: bool,
}

impl<T: Fetch> Fetcher<T> {
    /// Create a new Scores stream.
    pub async fn new(client: OsuClient, fetcher: T) -> Result<Self> {
        let mut s = Self {
            fetcher,
            client,
            items: Vec::new(),
            more_exists: true,
        };
        // fetch the first page immediately.
        s.fetch_next_page().await?;
        Ok(s)
    }
}

impl<T: Fetch> LazyBuffer<T::Item> for Fetcher<T> {
    /// Total length of the pages.
    fn length_fetched(&self) -> Size {
        let count = self.len();
        if self.more_exists {
            Size::AtLeast(count)
        } else {
            Size::Total(count)
        }
    }

    fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Get the index-th score.
    async fn get(&mut self, index: usize) -> Result<Option<&T::Item>> {
        Ok(self.get_range(index..(index + 1)).await?.first())
    }

    /// Get all scores.
    async fn get_all(mut self) -> Result<Vec<T::Item>> {
        let _ = self.get_range(0..usize::MAX).await?;
        Ok(self.items)
    }

    /// Get the scores between the given range.
    async fn get_range(&mut self, range: Range<usize>) -> Result<&[T::Item]> {
        while self.len() < range.end {
            if !self.fetch_next_page().await? {
                break;
            }
        }
        Ok(&self.items[fit_range_to_len(self.len(), range)])
    }

    async fn find<F: FnMut(&T::Item) -> bool + Send>(
        &mut self,
        mut f: F,
    ) -> Result<Option<&T::Item>> {
        let mut from = 0usize;
        let index = loop {
            if from == self.len() && !self.fetch_next_page().await? {
                break None;
            }
            if f(&self.items[from]) {
                break Some(from);
            }
            from += 1;
        };
        Ok(index.map(|v| &self.items[v]))
    }
}

impl<T: Fetch> Fetcher<T> {
    async fn fetch_next_page(&mut self) -> Result<bool> {
        if !self.more_exists {
            return Ok(false);
        }
        let offset = self.len();
        let scores = self.fetcher.fetch(&self.client, offset).await?;
        if scores.len() < T::ITEMS_PER_PAGE {
            self.more_exists = false;
        }
        if scores.is_empty() {
            return Ok(false);
        }
        self.items.extend(scores);
        Ok(true)
    }
    fn len(&self) -> usize {
        self.items.len()
    }
}
