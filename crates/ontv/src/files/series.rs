use std::collections::{BTreeSet, HashMap};

use crate::files::iter::Iter;
use crate::model::{Series, SeriesId};

#[derive(Default)]
pub(crate) struct Database {
    // Series indexed by id.
    data: HashMap<SeriesId, Series>,
    // Series indexed by name.
    by_name: BTreeSet<(String, SeriesId)>,
}

impl Database {
    /// Get a series immutably.
    pub(crate) fn get(&self, id: &SeriesId) -> Option<&Series> {
        self.data.get(id)
    }

    /// Get a series mutably.
    pub(crate) fn get_mut(&mut self, id: &SeriesId) -> Option<&mut Series> {
        self.data.get_mut(id)
    }

    /// Remove the series with the given identifier.
    pub(crate) fn remove(&mut self, id: &SeriesId) -> Option<Series> {
        let series = self.data.remove(id)?;
        let _ = self.by_name.remove(&(series.title.clone(), series.id));
        Some(series)
    }

    /// Insert the given series.
    pub(crate) fn insert(&mut self, series: Series) {
        self.by_name.insert((series.title.clone(), series.id));
        self.data.insert(series.id, series);
    }

    /// Iterate over all series in the database in some random.
    pub(crate) fn iter(&self) -> impl ExactSizeIterator<Item = &Series> {
        self.data.values()
    }

    /// Iterate over all series in the database in some order.
    pub(crate) fn iter_by_name(&self) -> impl DoubleEndedIterator<Item = &Series> {
        Iter::new(self.by_name.iter().map(|(_, key)| key), &self.data)
    }

    /// Export series data.
    pub(crate) fn export(&self) -> impl IntoIterator<Item = Series> + 'static {
        let mut out = Vec::with_capacity(self.by_name.len());

        for (_, id) in &self.by_name {
            if let Some(series) = self.data.get(id) {
                out.push(series.clone());
            }
        }

        out
    }
}
