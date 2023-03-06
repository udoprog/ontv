use core::fmt;
use std::collections::HashMap;
use std::ops::Deref;

use crate::model::{Episode, EpisodeId, SeriesId};

struct EpisodeData {
    episode: Episode,
    prev: Option<EpisodeId>,
    next: Option<EpisodeId>,
}

impl EpisodeData {
    fn into_ref<'a>(&'a self, data: &'a HashMap<EpisodeId, EpisodeData>) -> EpisodeRef<'a> {
        EpisodeRef {
            episode: &self.episode,
            prev: self.prev,
            next: self.next,
            data,
        }
    }
}

/// A reference to an episode.
#[derive(Clone, Copy)]
pub(crate) struct EpisodeRef<'a> {
    episode: &'a Episode,
    prev: Option<EpisodeId>,
    next: Option<EpisodeId>,
    data: &'a HashMap<EpisodeId, EpisodeData>,
}

impl fmt::Debug for EpisodeRef<'_> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EpisodeRef")
            .field("episode", &self.episode)
            .field("prev", &self.prev)
            .field("next", &self.next)
            .finish_non_exhaustive()
    }
}

impl<'a> EpisodeRef<'a> {
    #[inline]
    pub(crate) fn next(self) -> Option<EpisodeRef<'a>> {
        let data = self.data.get(&self.next?)?;
        Some(data.into_ref(self.data))
    }
}

impl<'a> Deref for EpisodeRef<'a> {
    type Target = Episode;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.episode
    }
}

#[derive(Default, Clone, Copy)]
struct SeriesData {
    first: Option<EpisodeId>,
    last: Option<EpisodeId>,
    len: usize,
}

#[derive(Default)]
pub(crate) struct Database {
    /// Episode data.
    data: HashMap<EpisodeId, EpisodeData>,
    /// By series index.
    by_series: HashMap<SeriesId, SeriesData>,
}

impl Database {
    /// Insert a database.
    pub(crate) fn insert(&mut self, id: SeriesId, episodes: Vec<Episode>) {
        let len = episodes.len();
        let mut first = None;
        let mut prev = None;

        let mut it = episodes.into_iter().peekable();

        while let Some(episode) = it.next() {
            let next = it.peek().map(|e| e.id);
            let id = episode.id;

            let links = EpisodeData {
                episode,
                prev,
                next,
            };

            self.data.insert(id, links);
            prev = Some(id);

            if first.is_none() {
                first = Some(id);
            }
        }

        self.by_series.insert(
            id,
            SeriesData {
                first,
                last: prev,
                len,
            },
        );
    }

    /// Remove a series by id.
    pub(crate) fn remove(&mut self, id: &SeriesId) {
        let Some(data) = self.by_series.remove(id) else {
            return;
        };

        let mut cur = data.first;

        while let Some(id) = cur.take() {
            cur = self.data.remove(&id).and_then(|data| data.next);
        }
    }

    /// Get an episode.
    pub(crate) fn get(&self, id: &EpisodeId) -> Option<EpisodeRef<'_>> {
        let data = self.data.get(id)?;

        Some(EpisodeRef {
            episode: &data.episode,
            prev: data.prev,
            next: data.next,
            data: &self.data,
        })
    }

    /// Get episodes by series.
    pub(crate) fn by_series<'a>(&'a self, id: &SeriesId) -> Iter<'_> {
        let state = self.by_series.get(id).copied().unwrap_or_default();

        Iter {
            head: state.first,
            tail: state.last,
            len: state.len,
            data: &self.data,
        }
    }
}

#[derive(Clone)]
pub(crate) struct Iter<'a> {
    head: Option<EpisodeId>,
    tail: Option<EpisodeId>,
    len: usize,
    data: &'a HashMap<EpisodeId, EpisodeData>,
}

impl Iter<'_> {
    /// Export remaining episodes in order.
    pub(crate) fn export(&self) -> Vec<Episode> {
        let mut data = Vec::with_capacity(self.len);

        for e in self.clone() {
            data.push(e.episode.clone());
        }

        data
    }
}

impl<'a> Iterator for Iter<'a> {
    type Item = EpisodeRef<'a>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let id = self.head.take()?;
        let data = self.data.get(&id)?;
        self.head = data.next;

        if self.tail == Some(id) {
            self.tail = None;
        }

        self.len = self.len.saturating_sub(1);
        Some(data.into_ref(self.data))
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}

impl<'a> DoubleEndedIterator for Iter<'a> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        let id = self.tail.take()?;
        let data = self.data.get(&id)?;
        self.tail = data.prev;

        if self.head == Some(id) {
            self.head = None;
        }

        self.len = self.len.saturating_sub(1);
        Some(data.into_ref(self.data))
    }
}

impl ExactSizeIterator for Iter<'_> {}
