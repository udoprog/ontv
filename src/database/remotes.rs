use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;

use parking_lot::Mutex;

use crate::model::{EpisodeId, MovieId, RemoteEpisodeId, RemoteId, RemoteIds, SeriesId};
use crate::utils::OptionIter;

#[derive(Default)]
struct Inner {
    series: BTreeMap<RemoteId, SeriesId>,
    movies: BTreeMap<RemoteId, MovieId>,
    episodes: BTreeMap<RemoteEpisodeId, EpisodeId>,
}

#[derive(Default)]
pub(crate) struct Database {
    inner: Arc<Mutex<Inner>>,
    by_series: HashMap<SeriesId, BTreeSet<RemoteId>>,
    by_movie: HashMap<MovieId, BTreeSet<RemoteId>>,
}

impl Database {
    /// Get a remote series identifier.
    pub(crate) fn get_series(&self, remote_id: &RemoteId) -> Option<SeriesId> {
        Some(*self.inner.lock().series.get(remote_id)?)
    }

    /// Get a remote movies identifier.
    pub(crate) fn get_movie(&self, remote_id: &RemoteId) -> Option<MovieId> {
        Some(*self.inner.lock().movies.get(remote_id)?)
    }

    /// Get remote by series.
    pub(crate) fn get_by_series(
        &self,
        series_id: &SeriesId,
    ) -> impl ExactSizeIterator<Item = RemoteId> + '_ + use<'_> {
        OptionIter::new(self.by_series.get(series_id).map(|it| it.iter())).copied()
    }

    /// Get remote by movie.
    pub(crate) fn get_by_movie(
        &self,
        series_id: &MovieId,
    ) -> impl ExactSizeIterator<Item = RemoteId> + '_ + use<'_> {
        OptionIter::new(self.by_movie.get(series_id).map(|it| it.iter())).copied()
    }

    /// Insert a series remote.
    pub(crate) fn insert_series(&mut self, remote_id: RemoteId, series_id: SeriesId) -> bool {
        let mut inner = self.inner.lock();
        let replaced = inner.series.insert(remote_id, series_id);

        self.by_series
            .entry(series_id)
            .or_default()
            .insert(remote_id);

        !matches!(replaced, Some(id) if id == series_id)
    }

    /// Insert a movie remote.
    pub(crate) fn insert_movie(&mut self, remote_id: RemoteId, movie_id: MovieId) -> bool {
        let mut inner = self.inner.lock();
        let replaced = inner.movies.insert(remote_id, movie_id);
        self.by_movie.entry(movie_id).or_default().insert(remote_id);
        !matches!(replaced, Some(id) if id == movie_id)
    }

    /// Insert an episode remote.
    pub(crate) fn insert_episode(
        &mut self,
        remote_id: RemoteEpisodeId,
        episode_id: EpisodeId,
    ) -> bool {
        let mut inner = self.inner.lock();
        let replaced = inner.episodes.insert(remote_id, episode_id);
        !matches!(replaced, Some(id) if id == episode_id)
    }

    /// Construct a data proxy used when querying remote sources.
    pub(crate) fn proxy(&self) -> Proxy {
        Proxy {
            inner: self.inner.clone(),
        }
    }

    /// Export the contents of the database.
    pub(crate) fn export(&self) -> impl IntoIterator<Item = RemoteIds> + 'static + use<> {
        let inner = self.inner.lock();

        let mut series = BTreeMap::<_, Vec<_>>::new();
        let mut movies = BTreeMap::<_, Vec<_>>::new();
        let mut episodes = BTreeMap::<_, Vec<_>>::new();

        for (&remote_id, &series_id) in &inner.series {
            series.entry(series_id).or_default().push(remote_id);
        }

        for (&remote_id, &movie_id) in &inner.movies {
            movies.entry(movie_id).or_default().push(remote_id);
        }

        for (&remote_id, &episode_id) in &inner.episodes {
            episodes.entry(episode_id).or_default().push(remote_id);
        }

        let a = series
            .into_iter()
            .map(|(uuid, remotes)| RemoteIds::Series { uuid, remotes });

        let b = movies
            .into_iter()
            .map(|(uuid, remotes)| RemoteIds::Movies { uuid, remotes });

        let c = episodes
            .into_iter()
            .map(|(uuid, remotes)| RemoteIds::Episode { uuid, remotes });

        a.chain(b).chain(c)
    }
}

pub(crate) struct Proxy {
    inner: Arc<Mutex<Inner>>,
}

impl Proxy {
    /// Translate a remote to a series id.
    pub(crate) fn find_series_by_remote(&self, remote_id: RemoteId) -> Option<SeriesId> {
        self.inner.lock().series.get(&remote_id).copied()
    }

    /// Translate a remote to a movie id.
    pub(crate) fn find_movie_by_remote(&self, remote_id: RemoteId) -> Option<MovieId> {
        self.inner.lock().movies.get(&remote_id).copied()
    }

    /// Find episode by remote.
    pub(crate) fn find_episode_by_remote(&self, remote_id: RemoteEpisodeId) -> Option<EpisodeId> {
        self.inner.lock().episodes.get(&remote_id).copied()
    }
}
