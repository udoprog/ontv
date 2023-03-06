use std::collections::{btree_set, BTreeSet, HashMap};

use chrono::{DateTime, Utc};

use crate::model::{Pending, SeriesId};

#[derive(Default)]
pub(crate) struct Database {
    /// Pending by series.
    data: HashMap<SeriesId, Pending>,
    /// Index by timestamp.
    by_timestamp: BTreeSet<(DateTime<Utc>, SeriesId)>,
}

impl Database {
    /// Get an existing pending element.
    #[inline]
    pub(crate) fn get(&mut self, id: &SeriesId) -> Option<&Pending> {
        self.data.get(id)
    }

    /// Export data from the database.
    pub(crate) fn export(&self) -> impl IntoIterator<Item = Pending> {
        let mut export = Vec::with_capacity(self.by_timestamp.len());

        for (_, id) in &self.by_timestamp {
            if let Some(pending) = self.data.get(id) {
                export.push(pending.clone());
            }
        }

        export
    }

    /// Remove by previously looked up index.
    #[inline]
    pub(crate) fn remove(&mut self, id: &SeriesId) -> Option<Pending> {
        let p = self.data.remove(id)?;
        self.by_timestamp.remove(&(p.timestamp, p.series));
        Some(p)
    }

    /// Iterate immutably over pending entries in timestamp order.
    #[inline]
    pub(crate) fn iter(&self) -> impl DoubleEndedIterator<Item = &Pending> {
        Iter {
            iter: self.by_timestamp.iter(),
            data: &self.data,
        }
    }

    /// Get pending by series.
    #[inline]
    pub(crate) fn by_series(&self, id: &SeriesId) -> Option<&Pending> {
        self.data.get(id)
    }
}

impl Extend<Pending> for Database {
    fn extend<T>(&mut self, iter: T)
    where
        T: IntoIterator<Item = Pending>,
    {
        for p in iter {
            if let Some(p) = self.data.remove(&p.series) {
                let _ = self.by_timestamp.remove(&(p.timestamp, p.series));
            }

            self.by_timestamp.insert((p.timestamp, p.series));
            self.data.insert(p.series, p);
        }
    }
}

struct Iter<'a> {
    iter: btree_set::Iter<'a, (DateTime<Utc>, SeriesId)>,
    data: &'a HashMap<SeriesId, Pending>,
}

impl<'a> Iterator for Iter<'a> {
    type Item = &'a Pending;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let (_, id) = self.iter.next()?;
        self.data.get(id)
    }
}

impl DoubleEndedIterator for Iter<'_> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        let (_, id) = self.iter.next_back()?;
        self.data.get(id)
    }
}
