use std::collections::{BTreeMap, HashMap};

use uuid::Uuid;

use crate::model::{Episode, EpisodeId, SeriesId, Watched};

#[derive(Default)]
pub(crate) struct Database {
    by_episode: BTreeMap<EpisodeId, Vec<Watched>>,
}

impl Database {
    pub(crate) fn get(&self, episode_id: &EpisodeId) -> &[Watched] {
        self.by_episode
            .get(episode_id)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    pub(crate) fn insert(&mut self, w: Watched) {
        self.by_episode.entry(w.episode).or_default().push(w);
    }

    /// Remove all episodes matching a series.
    pub(crate) fn remove_by_series(
        &mut self,
        series_id: &SeriesId,
        episodes: &HashMap<SeriesId, Vec<Episode>>,
    ) {
        let Some(episodes) = episodes.get(series_id) else {
            return;
        };

        for e in episodes {
            let _ = self.by_episode.remove(&e.id);
        }
    }

    /// Remove all watches related to an episode.
    pub(crate) fn remove(&mut self, episode_id: &EpisodeId) -> usize {
        let Some(removed) = self.by_episode.remove(episode_id) else {
            return 0;
        };

        removed.len()
    }

    /// Remove a single watch by id.
    pub(crate) fn remove_watch(&mut self, episode_id: &EpisodeId, watch_id: &Uuid) -> usize {
        let watches = self.by_episode.entry(*episode_id).or_default();
        watches.retain(|w| w.id != *watch_id);
        watches.len()
    }

    /// Construct an export of the watched database.
    pub(crate) fn export(&self) -> impl IntoIterator<Item = Watched> + 'static {
        self.by_episode.clone().into_iter().flat_map(|(_, v)| v)
    }
}
