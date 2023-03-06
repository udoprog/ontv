use std::collections::btree_map::BTreeMap;
use std::collections::hash_map::{self, HashMap};

use serde::Serialize;
use slab::Slab;
use uuid::Uuid;

use crate::database::episodes;
use crate::model::{EpisodeId, SeasonNumber, SeriesId, Watched, WatchedKind};

#[derive(Debug, Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Hash)]
pub(crate) enum Place {
    Episode(SeasonNumber, u32),
}

impl Serialize for Place {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Place::Episode(season, number) => {
                serializer.collect_str(&format_args!("{season}x{number}", season = season.short()))
            }
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct Export {
    place: Place,
    #[serde(flatten)]
    watched: Watched,
}

#[derive(Default)]
pub(crate) struct Database {
    data: Slab<Watched>,
    by_id: HashMap<Uuid, usize>,
    by_episode: HashMap<EpisodeId, Vec<usize>>,
    by_series: HashMap<SeriesId, Vec<usize>>,
}

impl Database {
    /// Get all watches for the given episode.
    pub(crate) fn by_episode(
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
    pub(crate) fn by_series(
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
        let kind = w.kind;
        let index = self.data.insert(w);
        self.by_id.insert(id, index);

        match kind {
            WatchedKind::Series { series, episode } => {
                self.by_episode.entry(episode).or_default().push(index);
                self.by_series.entry(series).or_default().push(index);
            }
        }
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

            match w.kind {
                WatchedKind::Series { episode, .. } => {
                    let _ = self.by_episode.remove(&episode);
                }
            }
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

            match w.kind {
                WatchedKind::Series { series, .. } => {
                    self.clear_series_by_id(&series, index);
                }
            }
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

        match w.kind {
            WatchedKind::Series { series, episode } => {
                self.clear_series_by_id(&series, index);
                self.clear_episode_by_id(&episode, index);
            }
        }

        Some(w)
    }

    /// Construct an export of the watched database.
    pub(crate) fn export(
        &self,
        episodes: &episodes::Database,
    ) -> impl IntoIterator<Item = Export> + 'static {
        let mut export = BTreeMap::new();

        for (_, w) in &self.data {
            let place = match &w.kind {
                WatchedKind::Series { episode, .. } => {
                    let (season, number) = episodes
                        .get(episode)
                        .map(|e| (e.season, e.number))
                        .unwrap_or_default();
                    Place::Episode(season, number)
                }
            };

            export.insert((w.timestamp, place, w.id), Export { watched: *w, place });
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
