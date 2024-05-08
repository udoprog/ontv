use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::database::iter::Iter;
use crate::model::{MovieId, Pending, SeriesId};

#[derive(Default)]
pub(crate) struct Database {
    /// Pending by series.
    data: BTreeMap<Uuid, Pending>,
    /// Index by timestamp.
    by_timestamp: BTreeSet<(DateTime<Utc>, Uuid)>,
}

impl Database {
    /// Get an existing pending element.
    #[inline]
    pub(crate) fn get(&self, id: &SeriesId) -> Option<&Pending> {
        self.data.get(id.id())
    }

    /// Export data from the database.
    pub(crate) fn export(&self) -> impl IntoIterator<Item = Pending> {
        let mut export = Vec::with_capacity(self.by_timestamp.len());

        for pending in self.data.values() {
            export.push(*pending);
        }

        export
    }

    /// Remove by series id.
    #[inline]
    pub(crate) fn remove_series(&mut self, id: &SeriesId) -> Option<Pending> {
        let p = self.data.remove(id.id())?;
        self.by_timestamp.remove(&(p.timestamp, *id.id()));
        Some(p)
    }

    /// Remove by movie id.
    #[inline]
    pub(crate) fn remove_movie(&mut self, id: &MovieId) -> Option<Pending> {
        let p = self.data.remove(id.id())?;
        self.by_timestamp.remove(&(p.timestamp, *id.id()));
        Some(p)
    }

    /// Iterate immutably over pending entries in timestamp order.
    #[inline]
    pub(crate) fn iter(&self) -> impl DoubleEndedIterator<Item = &Pending> + Clone {
        Iter::new(self.by_timestamp.iter().map(|(_, key)| key), &self.data)
    }

    /// Get pending by movie id.
    #[inline]
    pub(crate) fn by_movie(&self, id: &MovieId) -> Option<&Pending> {
        self.data.get(id.id())
    }
}

impl Extend<Pending> for Database {
    fn extend<T>(&mut self, iter: T)
    where
        T: IntoIterator<Item = Pending>,
    {
        for p in iter {
            if let Some(p) = self.data.remove(p.id()) {
                let _ = self.by_timestamp.remove(&(p.timestamp, *p.id()));
            }

            self.by_timestamp.insert((p.timestamp, *p.id()));
            self.data.insert(*p.id(), p);
        }
    }
}
