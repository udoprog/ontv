use core::fmt;
use std::collections::HashMap;
use std::ops::Deref;

use crate::model::{Episode, EpisodeId, SeriesId};
use crate::prelude::SeasonNumber;

struct EpisodeData {
    episode: Episode,
    series: SeriesId,
    prev: Option<EpisodeId>,
    next: Option<EpisodeId>,
}

impl EpisodeData {
    fn as_episode_ref<'a>(&'a self, data: &'a HashMap<EpisodeId, EpisodeData>) -> EpisodeRef<'a> {
        EpisodeRef {
            episode: &self.episode,
            series: self.series,
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
    series: SeriesId,
    prev: Option<EpisodeId>,
    next: Option<EpisodeId>,
    data: &'a HashMap<EpisodeId, EpisodeData>,
}

impl EpisodeRef<'_> {
    /// Get series episode belongs to.
    pub(crate) fn series(&self) -> &SeriesId {
        &self.series
    }
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
        Some(data.as_episode_ref(self.data))
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
    /// Link to first episode in season.
    by_season: HashMap<(SeriesId, SeasonNumber), Vec<EpisodeId>>,
}

impl Database {
    /// Insert a database.
    #[tracing::instrument(skip(self, episodes))]
    pub(crate) fn insert(&mut self, series: SeriesId, mut episodes: Vec<Episode>) {
        episodes.sort_by_cached_key(|e| e.watch_order_key());

        let len = episodes.len();
        let mut first = None;
        let mut prev = None;

        self.remove(&series);

        let mut it = episodes.into_iter().peekable();

        while let Some(episode) = it.next() {
            let next = it.peek().map(|e| e.id);
            let id = episode.id;

            self.by_season
                .entry((series, episode.season))
                .or_default()
                .push(episode.id);

            let links = EpisodeData {
                episode,
                series,
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
            series,
            SeriesData {
                first,
                last: prev,
                len,
            },
        );
    }

    /// Remove a series by id.
    pub(crate) fn remove(&mut self, series_id: &SeriesId) {
        let Some(data) = self.by_series.remove(series_id) else {
            return;
        };

        let mut cur = data.first;

        while let Some(id) = cur.take() {
            if let Some(e) = self.data.remove(&id) {
                let _ = self.by_season.remove(&(*series_id, e.episode.season));
                cur = e.next;
            }
        }
    }

    /// Get an episode.
    pub(crate) fn get(&self, id: &EpisodeId) -> Option<EpisodeRef<'_>> {
        let data = self.data.get(id)?;
        Some(data.as_episode_ref(&self.data))
    }

    /// Get episodes by series.
    pub(crate) fn by_series(&self, id: &SeriesId) -> Iter<'_> {
        let state = self.by_series.get(id).copied().unwrap_or_default();

        Iter {
            head: state.first,
            tail: state.last,
            len: state.len,
            data: &self.data,
        }
    }

    /// Get episodes by season.
    pub(crate) fn by_season(
        &self,
        id: &SeriesId,
        season: &SeasonNumber,
    ) -> impl DoubleEndedIterator<Item = EpisodeRef<'_>> + ExactSizeIterator {
        let iter = self
            .by_season
            .get(&(*id, *season))
            .map(Vec::as_slice)
            .unwrap_or_default();

        crate::database::iter::Iter::new(iter.iter(), &self.data)
            .map(|e| e.as_episode_ref(&self.data))
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
        Some(data.as_episode_ref(self.data))
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
        Some(data.as_episode_ref(self.data))
    }
}

impl ExactSizeIterator for Iter<'_> {
    #[inline]
    fn len(&self) -> usize {
        self.len
    }
}
