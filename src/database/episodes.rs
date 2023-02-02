use std::collections::HashMap;

use crate::model::{Episode, EpisodeId, SeriesId};

#[derive(Default)]
pub(crate) struct Database {
    by_series: HashMap<SeriesId, Vec<Episode>>,
    by_episode: HashMap<EpisodeId, (SeriesId, usize)>,
}

impl Database {
    /// Get by series id.
    pub(crate) fn get(&self, series_id: &SeriesId) -> &[Episode] {
        let Some(episodes) = self.by_series.get(series_id) else {
            return &[];
        };

        episodes
    }

    /// Get episode.
    pub(crate) fn get_by_episode(&self, episode: &EpisodeId) -> Option<&Episode> {
        let (series_id, index) = self.by_episode.get(episode)?;
        self.by_series.get(series_id)?.get(*index)
    }

    /// Remove the given series.
    pub(crate) fn remove(&mut self, series_id: &SeriesId) {
        if let Some(episodes) = self.by_series.remove(series_id) {
            for e in episodes {
                self.by_episode.remove(&e.id);
            }
        }
    }

    /// Replace episodes for the given series.
    pub(crate) fn insert(&mut self, series_id: &SeriesId, episodes: Vec<Episode>) {
        if let Some(episodes) = self.by_series.get(series_id) {
            for e in episodes {
                self.by_episode.remove(&e.id);
            }
        }

        for (index, e) in episodes.iter().enumerate() {
            self.by_episode.insert(e.id, (*series_id, index));
        }

        self.by_series.insert(*series_id, episodes);
    }

    /// Get series by episode.
    pub(crate) fn series_by_episode(&self, id: &EpisodeId) -> Option<&SeriesId> {
        Some(&self.by_episode.get(id)?.0)
    }
}
