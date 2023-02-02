use std::collections::{BTreeMap, HashMap};

use chrono::{DateTime, Utc};
use slab::Slab;

use crate::model::{EpisodeId, Pending, SeriesId};

pub(crate) struct Index(SeriesId, usize);

#[derive(Clone)]
pub(crate) struct Data {
    series: SeriesId,
    pending: Pending,
}

#[derive(Default)]
pub(crate) struct Database {
    data: Slab<Data>,
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
            .map(move |index| data.remove(index).pending)
    }

    /// Insert by series.
    pub(crate) fn insert(&mut self, series: SeriesId, pending: Pending) {
        let by_timestamp = (pending.timestamp, series);

        let index = self.data.insert(Data { series, pending });

        self.by_series.insert(series, index);
        self.by_timestamp.insert(by_timestamp, index);
    }

    /// Get a stable position for the pending episode related to the given series.
    ///
    /// The position will only be stable for as long as the database hasn't been modified.
    #[inline]
    pub(crate) fn position_for_series(&self, series_id: &SeriesId) -> Option<Index> {
        Some(Index(*series_id, *self.by_series.get(series_id)?))
    }

    /// Remove by previously looked up index.
    #[inline]
    pub(crate) fn remove_by_index(&mut self, Index(series_id, index): Index) -> Option<Pending> {
        let data = self.data.try_remove(index)?;
        self.by_series.remove(&series_id);
        self.by_timestamp
            .remove(&(data.pending.timestamp, series_id));
        Some(data.pending)
    }

    /// Remove pending by predicate.
    #[inline]
    pub(crate) fn remove_by_episode(&mut self, episode_id: &EpisodeId) {
        let mut removed = Vec::new();

        for (index, p) in &self.data {
            if p.pending.episode == *episode_id {
                removed.push(Index(p.series, index));
            }
        }

        for index in removed {
            self.remove_by_index(index);
        }
    }

    /// Remove pending by predicate.
    #[inline]
    pub(crate) fn remove_by_series(&mut self, series_id: &SeriesId) {
        let Some(index) = self.by_series.get(series_id) else {
            return;
        };

        self.remove_by_index(Index(*series_id, *index));
    }

    /// Iterate immutably over pending entries in timestamp order.
    #[inline]
    pub(crate) fn iter(&self) -> impl DoubleEndedIterator<Item = &Pending> {
        self.by_timestamp
            .values()
            .map(|&index| &self.data[index].pending)
    }

    /// Iterate mutably over data.
    #[inline]
    pub(crate) fn iter_mut(&mut self) -> impl DoubleEndedIterator<Item = &mut Pending> {
        self.data.iter_mut().map(|(_, data)| &mut data.pending)
    }

    /// Get pending by series.
    #[inline]
    pub(crate) fn by_series(&self, series_id: &SeriesId) -> Option<&Pending> {
        let index = *self.by_series.get(series_id)?;
        Some(&self.data.get(index)?.pending)
    }

    /// Modify pending by index in place.
    #[inline]
    pub(crate) fn modify<M>(&mut self, Index(_, index): Index, mut modify: M)
    where
        M: FnMut(&mut Pending),
    {
        let Some(data) = self.data.get_mut(index) else {
            return;
        };

        let old_by_timestamp = (data.pending.timestamp, data.series);

        modify(&mut data.pending);

        let new_by_timestamp = (data.pending.timestamp, data.series);

        if new_by_timestamp != old_by_timestamp {
            self.by_timestamp.remove(&old_by_timestamp);
            self.by_timestamp.insert(new_by_timestamp, index);
        }
    }
}
