use std::collections::btree_map::BTreeMap;
use std::collections::hash_map::{self, HashMap};

use serde::Serialize;

use crate::database::episodes;
use crate::database::iter::Iter;
use crate::model::{EpisodeId, SeasonNumber, SeriesId, Watched, WatchedId, WatchedKind};

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
    data: HashMap<WatchedId, Watched>,
    by_episode: HashMap<EpisodeId, Vec<WatchedId>>,
    by_series: HashMap<SeriesId, Vec<WatchedId>>,
}

impl Database {
    /// Get all watches for the given episode.
    pub(crate) fn by_episode(
        &self,
        id: &EpisodeId,
    ) -> impl ExactSizeIterator<Item = &Watched> + DoubleEndedIterator + Clone {
        let indexes = self
            .by_episode
            .get(id)
            .map(Vec::as_slice)
            .unwrap_or_default();

        Iter::new(indexes.iter(), &self.data)
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

        Iter::new(indexes.iter(), &self.data)
    }

    /// Insert a new entry into watch history.
    pub(crate) fn insert(&mut self, w: Watched) {
        let id = w.id;
        let kind = w.kind;

        if let Some(w) = self.data.insert(id, w) {
            match &w.kind {
                WatchedKind::Series { series, episode } => {
                    self.clear_series_by_id(series, &w.id);
                    self.clear_episode_by_id(episode, &w.id);
                }
            }
        }

        match kind {
            WatchedKind::Series { series, episode } => {
                self.by_episode.entry(episode).or_default().push(w.id);
                self.by_series.entry(series).or_default().push(w.id);
            }
        }
    }

    /// Remove all episodes matching a series.
    pub(crate) fn remove_by_series(&mut self, series_id: &SeriesId) {
        let Some(indexes) = self.by_series.remove(series_id) else {
            return;
        };

        for id in indexes {
            let Some(w) = self.data.remove(&id) else {
                continue;
            };

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

        for id in removed {
            let Some(w) = self.data.remove(&id) else {
                continue;
            };

            match w.kind {
                WatchedKind::Series { series, .. } => {
                    self.clear_series_by_id(&series, &w.id);
                }
            }
        }

        len
    }

    /// Remove a single watch by id.
    pub(crate) fn remove_watch(&mut self, id: &WatchedId) -> Option<Watched> {
        let w = self.data.remove(id)?;

        match w.kind {
            WatchedKind::Series { series, episode } => {
                self.clear_series_by_id(&series, &w.id);
                self.clear_episode_by_id(&episode, &w.id);
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

    fn clear_series_by_id(&mut self, series_id: &SeriesId, id: &WatchedId) {
        let hash_map::Entry::Occupied(mut e) = self.by_series.entry(*series_id) else {
            return;
        };

        e.get_mut().retain(|&this| this != *id);

        if e.get().is_empty() {
            e.remove();
        }
    }

    fn clear_episode_by_id(&mut self, episode_id: &EpisodeId, id: &WatchedId) {
        let hash_map::Entry::Occupied(mut e) = self.by_episode.entry(*episode_id) else {
            return;
        };

        e.get_mut().retain(|&this| this != *id);

        if e.get().is_empty() {
            e.remove();
        }
    }
}
