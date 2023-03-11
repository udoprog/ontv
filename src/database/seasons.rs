use core::fmt;
use std::collections::HashMap;
use std::ops::Deref;

use crate::model::{Season, SeasonNumber, SeriesId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Pointer {
    series: SeriesId,
    number: SeasonNumber,
}

impl Pointer {
    fn new(series: SeriesId, number: SeasonNumber) -> Self {
        Self { series, number }
    }
}

struct SeasonData {
    season: Season,
    series: SeriesId,
    prev: Option<Pointer>,
    next: Option<Pointer>,
}

impl SeasonData {
    fn into_ref<'a>(&'a self) -> SeasonRef<'a> {
        SeasonRef {
            season: &self.season,
            series: self.series,
        }
    }
}

/// A reference to an season.
#[derive(Clone, Copy)]
pub(crate) struct SeasonRef<'a> {
    season: &'a Season,
    series: SeriesId,
}

impl<'a> SeasonRef<'a> {
    /// Get series season belongs to.
    pub(crate) fn series(&self) -> &SeriesId {
        &self.series
    }

    /// Coerce into season.
    pub(crate) fn into_season(self) -> &'a Season {
        self.season
    }
}

impl fmt::Debug for SeasonRef<'_> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SeasonRef")
            .field("season", &self.season)
            .finish_non_exhaustive()
    }
}

impl<'a> Deref for SeasonRef<'a> {
    type Target = Season;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.season
    }
}

#[derive(Default, Clone, Copy)]
struct SeriesData {
    first: Option<Pointer>,
    last: Option<Pointer>,
    len: usize,
}

#[derive(Default)]
pub(crate) struct Database {
    /// Season data.
    data: HashMap<Pointer, SeasonData>,
    /// By series index.
    by_series: HashMap<SeriesId, SeriesData>,
}

impl Database {
    /// Insert a database.
    #[tracing::instrument(skip(self, seasons))]
    pub(crate) fn insert(&mut self, series: SeriesId, seasons: Vec<Season>) {
        let len = seasons.len();
        let mut first = None;
        let mut prev = None;

        let _ = self.remove(&series);

        let mut it = seasons.into_iter().peekable();

        while let Some(season) = it.next() {
            let next = it.peek().map(|s| Pointer::new(series, s.number));
            let id = Pointer::new(series, season.number);

            let links = SeasonData {
                season,
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
    pub(crate) fn remove(&mut self, id: &SeriesId) {
        let Some(data) = self.by_series.remove(id) else {
            return;
        };

        let mut cur = data.first;

        while let Some(id) = cur.take() {
            cur = self.data.remove(&id).and_then(|data| data.next);
        }
    }

    /// Get an season.
    pub(crate) fn get(&self, series_id: &SeriesId, season: &SeasonNumber) -> Option<SeasonRef<'_>> {
        let data = self.data.get(&Pointer::new(*series_id, *season))?;
        Some(data.into_ref())
    }

    /// Get seasons by series.
    pub(crate) fn by_series(&self, id: &SeriesId) -> Iter<'_> {
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
    head: Option<Pointer>,
    tail: Option<Pointer>,
    len: usize,
    data: &'a HashMap<Pointer, SeasonData>,
}

impl Iter<'_> {
    /// Export remaining seasons in order.
    pub(crate) fn export(&self) -> Vec<Season> {
        let mut data = Vec::with_capacity(self.len);

        for e in self.clone() {
            data.push(e.season.clone());
        }

        data
    }
}

impl<'a> Iterator for Iter<'a> {
    type Item = SeasonRef<'a>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let id = self.head.take()?;
        let data = self.data.get(&id)?;

        if self.tail == Some(id) {
            self.tail = None;
        } else {
            self.head = data.next;
        }

        self.len = self.len.saturating_sub(1);
        Some(data.into_ref())
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

        if self.head == Some(id) {
            self.head = None;
        } else {
            self.tail = data.prev;
        }

        self.len = self.len.saturating_sub(1);
        Some(data.into_ref())
    }
}

impl ExactSizeIterator for Iter<'_> {
    #[inline]
    fn len(&self) -> usize {
        self.len
    }
}
