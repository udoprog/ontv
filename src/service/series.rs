use std::collections::HashMap;

use crate::model::{Series, SeriesId};

#[derive(Default)]
pub(crate) struct Database {
    data: Vec<Series>,
    by_id: HashMap<SeriesId, usize>,
}

impl Database {
    /// Get a series immutably.
    pub(crate) fn get(&self, id: &SeriesId) -> Option<&Series> {
        let &index = self.by_id.get(id)?;
        self.data.get(index)
    }

    /// Get a series mutably.
    pub(crate) fn get_mut(&mut self, id: &SeriesId) -> Option<&mut Series> {
        let &index = self.by_id.get(id)?;
        self.data.get_mut(index)
    }

    /// Remove the series by the given identifier.
    pub(crate) fn remove(&mut self, id: &SeriesId) -> Option<Series> {
        let index = self.by_id.remove(id)?;
        let value = self.data.swap_remove(index);
        let data = &mut self.data[index..];

        data.sort_by(|a, b| a.title.cmp(&b.title));

        for (n, s) in data.iter().enumerate() {
            self.by_id.insert(s.id, index + n);
        }

        Some(value)
    }

    /// Insert the given series.
    pub(crate) fn push(&mut self, series: Series) {
        self.data.push(series);
        self.data.sort_by(|a, b| a.title.cmp(&b.title));
        self.by_id.clear();

        for (index, s) in self.data.iter().enumerate() {
            self.by_id.insert(s.id, index);
        }
    }

    /// Raw push.
    pub(crate) fn push_raw(&mut self, series: Series) {
        let len = self.data.len();
        self.by_id.insert(series.id, len);
        self.data.push(series);
    }

    /// Iterate over all series in the database.
    pub(crate) fn iter(&self) -> impl ExactSizeIterator<Item = &Series> {
        self.data.iter()
    }

    /// Iterate over all series mutably in the database.
    pub(crate) fn iter_mut(&mut self) -> impl ExactSizeIterator<Item = &mut Series> {
        self.data.iter_mut()
    }

    /// Export series data.
    pub(crate) fn export(&self) -> impl IntoIterator<Item = Series> + 'static {
        self.data.clone()
    }
}
