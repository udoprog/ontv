use std::collections::btree_map::BTreeMap;
use std::collections::hash_map::{self, HashMap};

use slab::Slab;
use uuid::Uuid;

use crate::model::{EpisodeId, SeriesId, Watched};

#[derive(Default)]
pub(crate) struct Database {
    data: Slab<Watched>,
    by_id: HashMap<Uuid, usize>,
    by_episode: HashMap<EpisodeId, Vec<usize>>,
    by_series: HashMap<SeriesId, Vec<usize>>,
}

impl Database {
    /// Get all watches for the given episode.
    pub(crate) fn get(
        &self,
        episode_id: &EpisodeId,
    ) -> impl ExactSizeIterator<Item = &Watched> + DoubleEndedIterator + Clone {
        let indexes = self
            .by_episode
            .get(episode_id)
            .map(Vec::as_slice)
            .unwrap_or_default();

        indexes.iter().map(|&index| &self.data[index])
    }

    /// Get all watches for the given series.
    pub(crate) fn series(
        &self,
        series_id: &SeriesId,
    ) -> impl ExactSizeIterator<Item = &Watched> + DoubleEndedIterator + Clone {
        let indexes = self
            .by_series
            .get(series_id)
            .map(Vec::as_slice)
            .unwrap_or_default();

        indexes.iter().map(|&index| &self.data[index])
    }

    /// Insert a new entry into watch history.
    pub(crate) fn insert(&mut self, w: Watched) {
        let id = w.id;
        let episode_id = w.episode;
        let series_id = w.series;
        let index = self.data.insert(w);
        self.by_id.insert(id, index);
        self.by_episode.entry(episode_id).or_default().push(index);
        self.by_series.entry(series_id).or_default().push(index);
    }

    /// Remove all episodes matching a series.
    pub(crate) fn remove_by_series(&mut self, series_id: &SeriesId) {
        let Some(indexes) = self.by_series.remove(series_id) else {
            return;
        };

        for index in indexes {
            let Some(w) = self.data.try_remove(index) else {
                continue;
            };

            let _ = self.by_id.remove(&w.id);
            let _ = self.by_episode.remove(&w.episode);
        }
    }

    /// Remove all watches related to an episode.
    pub(crate) fn remove_by_episode(&mut self, episode_id: &EpisodeId) -> usize {
        let Some(removed) = self.by_episode.remove(episode_id) else {
            return 0;
        };

        let len = removed.len();

        for index in removed {
            let Some(w) = self.data.try_remove(index) else {
                continue;
            };

            let _ = self.by_id.remove(&w.id);
            self.clear_series_by_id(&w.series, index);
        }

        len
    }

    /// Remove a single watch by id.
    pub(crate) fn remove_watch(&mut self, id: &Uuid) -> Option<Watched> {
        let Some(index) = self.by_id.remove(id) else {
            return None;
        };

        let Some(w) = self.data.try_remove(index) else {
            return None;
        };

        self.clear_series_by_id(&w.series, index);
        self.clear_episode_by_id(&w.episode, index);
        Some(w)
    }

    /// Construct an export of the watched database.
    pub(crate) fn export(&self) -> impl IntoIterator<Item = Watched> + 'static {
        let mut export = BTreeMap::new();

        for (_, w) in &self.data {
            export.insert((w.timestamp, w.id), *w);
        }

        export.into_values()
    }

    fn clear_series_by_id(&mut self, series_id: &SeriesId, index: usize) {
        let hash_map::Entry::Occupied(mut e) = self.by_series.entry(*series_id) else {
            return;
        };

        e.get_mut().retain(|&this| this != index);

        if e.get().is_empty() {
            e.remove();
        }
    }

    fn clear_episode_by_id(&mut self, episode_id: &EpisodeId, index: usize) {
        let hash_map::Entry::Occupied(mut e) = self.by_episode.entry(*episode_id) else {
            return;
        };

        e.get_mut().retain(|&this| this != index);

        if e.get().is_empty() {
            e.remove();
        }
    }
}
