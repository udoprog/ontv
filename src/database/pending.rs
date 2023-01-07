use std::collections::{BTreeMap, HashMap};

use chrono::{DateTime, Utc};
use slab::Slab;

use crate::model::{Pending, SeriesId};

#[repr(transparent)]
pub(crate) struct Index(usize);

#[derive(Default)]
pub(crate) struct Database {
    data: Slab<Pending>,
    by_timestamp: BTreeMap<(DateTime<Utc>, SeriesId), usize>,
    by_series: HashMap<SeriesId, usize>,
}

impl Database {
    /// Export data from the database.
    pub(crate) fn export(&self) -> impl IntoIterator<Item = Pending> {
        let mut data = self.data.clone();
        self.by_timestamp
            .clone()
            .into_values()
            .map(move |index| data.remove(index))
    }

    /// Get a stable position for the pending episode related to the given series.
    ///
    /// The position will only be stable for as long as the database hasn't been modified.
    #[inline]
    pub(crate) fn position_for_series(&self, id: &SeriesId) -> Option<Index> {
        Some(Index(*self.by_series.get(id)?))
    }

    /// Remove by previously looked up index.
    #[inline]
    pub(crate) fn remove_by_index(&mut self, index: Index) -> Option<Pending> {
        let p = self.data.try_remove(index.0)?;
        self.by_series.remove(&p.series);
        self.by_timestamp.remove(&(p.timestamp, p.series));
        Some(p)
    }

    /// Remove pending by predicate.
    #[inline]
    pub(crate) fn remove_by<P>(&mut self, mut predicate: P)
    where
        P: FnMut(&Pending) -> bool,
    {
        let mut removed = Vec::new();

        for (index, p) in &self.data {
            if predicate(p) {
                removed.push(Index(index));
            }
        }

        for index in removed {
            self.remove_by_index(index);
        }
    }

    /// Iterate immutably over pending entries in timestamp order.
    #[inline]
    pub(crate) fn iter(&self) -> impl DoubleEndedIterator<Item = &Pending> {
        self.by_timestamp.values().map(|&index| &self.data[index])
    }

    /// Iterate mutably over data.
    #[inline]
    pub(crate) fn iter_mut(&mut self) -> impl DoubleEndedIterator<Item = &mut Pending> {
        self.data.iter_mut().map(|(_, value)| value)
    }

    /// Get pending by series.
    #[inline]
    pub(crate) fn by_series(&self, series_id: &SeriesId) -> Option<&Pending> {
        let index = *self.by_series.get(series_id)?;
        self.data.get(index)
    }

    /// Modify pending by index in place.
    #[inline]
    pub(crate) fn modify<M>(&mut self, index: Index, mut modify: M)
    where
        M: FnMut(&mut Pending),
    {
        let Some(p) = self.data.get_mut(index.0) else {
            return;
        };

        let old_by_timestamp = (p.timestamp, p.series);
        let old_by_series = p.series;

        modify(p);

        if p.series != old_by_series {
            self.by_series.remove(&old_by_series);
            self.by_series.insert(p.series, index.0);
        }

        let new_by_timestamp = (p.timestamp, p.series);

        if new_by_timestamp != old_by_timestamp {
            self.by_timestamp.remove(&old_by_timestamp);
            self.by_timestamp.insert(new_by_timestamp, index.0);
        }
    }
}

impl Extend<Pending> for Database {
    fn extend<T>(&mut self, iter: T)
    where
        T: IntoIterator<Item = Pending>,
    {
        for p in iter {
            let by_timestamp = (p.timestamp, p.series);
            let by_series = p.series;
            let index = self.data.insert(p);
            self.by_series.insert(by_series, index);
            self.by_timestamp.insert(by_timestamp, index);
        }
    }
}
