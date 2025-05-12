use std::{fmt::Display, future::Future, ops::Range};

use youmubot_prelude::*;

use crate::{models::Score, OsuClient};

pub const MAX_SCORE_PER_PAGE: usize = 1000;

/// Fetch scores given an offset.
/// Implemented for score requests.
pub trait FetchScores: Send {
    /// Scores per page.
    const SCORES_PER_PAGE: usize = MAX_SCORE_PER_PAGE;
    /// Fetch scores given an offset.
    fn fetch_scores(
        &self,
        client: &crate::OsuClient,
        offset: usize,
    ) -> impl Future<Output = Result<Vec<Score>>> + Send;
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
pub trait Scores: Send {
    /// Total length of the pages.
    fn length_fetched(&self) -> Size;

    /// Whether the scores set is empty.
    fn is_empty(&self) -> bool;

    /// Get the index-th score.
    fn get(&mut self, index: usize) -> impl Future<Output = Result<Option<&Score>>> + Send;

    /// Get all scores.
    fn get_all(self) -> impl Future<Output = Result<Vec<Score>>> + Send;

    /// Get the scores between the given range.
    fn get_range(&mut self, range: Range<usize>) -> impl Future<Output = Result<&[Score]>> + Send;

    /// Find a score that matches the predicate `f`.
    fn find<F: FnMut(&Score) -> bool + Send>(
        &mut self,
        f: F,
    ) -> impl Future<Output = Result<Option<&Score>>> + Send;
}

impl Scores for Vec<Score> {
    fn length_fetched(&self) -> Size {
        Size::Total(self.len())
    }

    fn is_empty(&self) -> bool {
        self.is_empty()
    }

    fn get(&mut self, index: usize) -> impl Future<Output = Result<Option<&Score>>> + Send {
        future::ok(self[..].get(index))
    }

    fn get_all(self) -> impl Future<Output = Result<Vec<Score>>> + Send {
        future::ok(self)
    }

    fn get_range(&mut self, range: Range<usize>) -> impl Future<Output = Result<&[Score]>> + Send {
        future::ok(&self[fit_range_to_len(self.len(), range)])
    }

    async fn find<F: FnMut(&Score) -> bool + Send>(&mut self, mut f: F) -> Result<Option<&Score>> {
        Ok(self.iter().find(|v| f(v)))
    }
}

#[inline]
fn fit_range_to_len(len: usize, range: Range<usize>) -> Range<usize> {
    range.start.min(len)..range.end.min(len)
}

/// A scores stream with a fetcher.
pub(super) struct ScoresFetcher<T> {
    fetcher: T,
    client: OsuClient,
    scores: Vec<Score>,
    more_exists: bool,
}

impl<T: FetchScores> ScoresFetcher<T> {
    /// Create a new Scores stream.
    pub async fn new(client: OsuClient, fetcher: T) -> Result<Self> {
        let mut s = Self {
            fetcher,
            client,
            scores: Vec::new(),
            more_exists: true,
        };
        // fetch the first page immediately.
        s.fetch_next_page().await?;
        Ok(s)
    }
}

impl<T: FetchScores> Scores for ScoresFetcher<T> {
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
        self.scores.is_empty()
    }

    /// Get the index-th score.
    async fn get(&mut self, index: usize) -> Result<Option<&Score>> {
        Ok(self.get_range(index..(index + 1)).await?.first())
    }

    /// Get all scores.
    async fn get_all(mut self) -> Result<Vec<Score>> {
        let _ = self.get_range(0..usize::MAX).await?;
        Ok(self.scores)
    }

    /// Get the scores between the given range.
    async fn get_range(&mut self, range: Range<usize>) -> Result<&[Score]> {
        while self.len() < range.end {
            if !self.fetch_next_page().await? {
                break;
            }
        }
        Ok(&self.scores[fit_range_to_len(self.len(), range)])
    }

    async fn find<F: FnMut(&Score) -> bool + Send>(&mut self, mut f: F) -> Result<Option<&Score>> {
        let mut from = 0usize;
        let index = loop {
            if from == self.len() && !self.fetch_next_page().await? {
                break None;
            }
            if f(&self.scores[from]) {
                break Some(from);
            }
            from += 1;
        };
        Ok(index.map(|v| &self.scores[v]))
    }
}

impl<T: FetchScores> ScoresFetcher<T> {
    async fn fetch_next_page(&mut self) -> Result<bool> {
        if !self.more_exists {
            return Ok(false);
        }
        let offset = self.len();
        let scores = self.fetcher.fetch_scores(&self.client, offset).await?;
        if scores.len() < T::SCORES_PER_PAGE {
            self.more_exists = false;
        }
        if scores.is_empty() {
            return Ok(false);
        }
        self.scores.extend(scores);
        Ok(true)
    }
    fn len(&self) -> usize {
        self.scores.len()
    }
}
