use std::collections::{btree_map, BTreeMap, HashMap};

use slab::Slab;

use crate::model::{Series, SeriesId};

#[derive(Default)]
pub(crate) struct Database {
    // Series storage.
    data: Slab<Series>,
    // Series indexed by id.
    by_id: HashMap<SeriesId, usize>,
    // Series indexed by name.
    by_name: BTreeMap<(String, SeriesId), usize>,
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

    /// Remove the series with the given identifier.
    pub(crate) fn remove(&mut self, id: &SeriesId) -> Option<Series> {
        let index = self.by_id.remove(id)?;
        let series = self.data.try_remove(index)?;
        let _ = self.by_name.remove(&(series.title.clone(), series.id));
        Some(series)
    }

    /// Insert the given series.
    pub(crate) fn insert(&mut self, series: Series) {
        let series_id = series.id;
        let series_title = series.title.clone();
        let index = self.data.insert(series);
        self.by_id.insert(series_id, index);
        self.by_name.insert((series_title, series_id), index);
    }

    /// Iterate over all series in the database in some order.
    pub(crate) fn iter(&self) -> impl ExactSizeIterator<Item = &Series> {
        self.data.iter().map(|(_, series)| series)
    }

    /// Iterate over all series in the database in some order.
    pub(crate) fn iter_by_name(&self) -> impl DoubleEndedIterator<Item = &Series> {
        self.by_name.iter().flat_map(|(_, &id)| self.data.get(id))
    }

    /// Iterate over all series mutably in the database.
    pub(crate) fn iter_mut(&mut self) -> impl ExactSizeIterator<Item = &mut Series> {
        self.data.iter_mut().map(|data| data.1)
    }

    /// Export series data.
    pub(crate) fn export(&self) -> impl IntoIterator<Item = Series> + 'static + Clone {
        Export {
            data: self.data.clone(),
            by_name: self.by_name.clone(),
        }
    }
}

#[derive(Clone)]
struct Export {
    data: Slab<Series>,
    by_name: BTreeMap<(String, SeriesId), usize>,
}

impl IntoIterator for Export {
    type Item = Series;
    type IntoIter = ExportFlatten<btree_map::IntoValues<(String, SeriesId), usize>>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        ExportFlatten {
            data: self.data,
            iter: self.by_name.clone().into_values(),
        }
    }
}

struct ExportFlatten<I> {
    data: Slab<Series>,
    iter: I,
}

impl<I> Iterator for ExportFlatten<I>
where
    I: Iterator<Item = usize>,
{
    type Item = Series;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let Some(series) = self.data.try_remove(self.iter.next()?) else {
                continue;
            };

            return Some(series);
        }
    }
}
