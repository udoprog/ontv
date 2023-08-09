use std::collections::{BTreeSet, HashMap};

use crate::database::iter::Iter;
use crate::model::{Movie, MovieId};

#[derive(Default)]
pub(crate) struct Database {
    // Movie indexed by id.
    data: HashMap<MovieId, Movie>,
    // Movie indexed by name.
    by_name: BTreeSet<(String, MovieId)>,
}

impl Database {
    /// Get a series immutably.
    pub(crate) fn get(&self, id: &MovieId) -> Option<&Movie> {
        self.data.get(id)
    }

    /// Get a series mutably.
    pub(crate) fn get_mut(&mut self, id: &MovieId) -> Option<&mut Movie> {
        self.data.get_mut(id)
    }

    /// Remove the series with the given identifier.
    pub(crate) fn remove(&mut self, id: &MovieId) -> Option<Movie> {
        let series = self.data.remove(id)?;
        let _ = self.by_name.remove(&(series.title.clone(), series.id));
        Some(series)
    }

    /// Insert the given series.
    pub(crate) fn insert(&mut self, series: Movie) {
        self.by_name.insert((series.title.clone(), series.id));
        self.data.insert(series.id, series);
    }

    /// Iterate over all series in the database in some order.
    pub(crate) fn iter_by_name(&self) -> impl DoubleEndedIterator<Item = &Movie> {
        Iter::new(self.by_name.iter().map(|(_, key)| key), &self.data)
    }

    /// Export series data.
    pub(crate) fn export(&self) -> impl IntoIterator<Item = Movie> + 'static {
        let mut out = Vec::with_capacity(self.by_name.len());

        for (_, id) in &self.by_name {
            if let Some(series) = self.data.get(id) {
                out.push(series.clone());
            }
        }

        out
    }
}
