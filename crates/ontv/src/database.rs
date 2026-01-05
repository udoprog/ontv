mod episodes;
mod format;
mod iter;
mod movies;
mod pending;
mod remotes;
mod seasons;
mod series;
mod sync;
mod watched;

use std::collections::HashSet;
use std::future::Future;
use std::path::Path;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use tracing_futures::Instrument;

pub(crate) use self::episodes::EpisodeRef;
pub(crate) use self::seasons::SeasonRef;
use crate::backend::paths;
use crate::model::{
    Config, Episode, Movie, MovieId, Pending, RemoteIds, Season, Series, SeriesId, Watched,
};
use crate::queue::Queue;

#[derive(Default)]
pub(crate) struct Database {
    /// Application configuration.
    pub(crate) config: Config,
    /// Remotes database.
    pub(crate) remotes: remotes::Database,
    /// Series database.
    pub(crate) series: series::Database,
    /// Movies database.
    pub(crate) movies: movies::Database,
    /// Episodes database.
    pub(crate) episodes: episodes::Database,
    /// Seasons collection.
    pub(crate) seasons: seasons::Database,
    /// Episode to watch history.
    pub(crate) watched: watched::Database,
    /// Ordered list of things to watch.
    pub(crate) pending: pending::Database,
    /// Synchronization state.
    pub(crate) sync: sync::Database,
    /// Keeping track of changes to be saved.
    pub(crate) changes: Changes,
    /// Download queue.
    pub(crate) tasks: Queue,
}

impl Database {
    /// Try to load initial state.
    pub(crate) fn load(paths: &paths::Paths) -> Result<Self> {
        let mut db = Self::default();

        if let Some((format, config)) =
            format::load(&paths.config).with_context(|| anyhow!("{}", paths.config.display()))?
        {
            db.config = config;

            if matches!(format, format::Format::Json) {
                db.changes.change(Change::Config);
            }
        }

        if let Some((format, remotes)) = format::load_array::<RemoteIds>(&paths.remotes)? {
            for remote_id in remotes {
                match remote_id {
                    RemoteIds::Series { uuid, remotes } => {
                        for remote_id in remotes {
                            db.remotes.insert_series(remote_id, uuid);
                        }
                    }
                    RemoteIds::Movies { uuid, remotes } => {
                        for remote_id in remotes {
                            db.remotes.insert_movie(remote_id, uuid);
                        }
                    }
                    RemoteIds::Episode { uuid, remotes } => {
                        for remote_id in remotes {
                            db.remotes.insert_episode(remote_id, uuid);
                        }
                    }
                }
            }

            if matches!(format, format::Format::Json) {
                db.changes.change(Change::Remotes);
            }
        }

        if let Some((_format, syncs)) = format::load_array::<sync::Export>(&paths.sync)? {
            for sync in syncs {
                db.sync.import_push(sync);
            }
        }

        if let Some((format, series)) = format::load_array::<Series>(&paths.series)? {
            for mut s in series {
                db.series.insert(s);
            }

            if matches!(format, format::Format::Json) {
                db.changes.change(Change::Series);
            }
        }

        if let Some((format, movies)) = format::load_array::<Movie>(&paths.movies)? {
            for s in movies {
                db.movies.insert(s);
            }

            if matches!(format, format::Format::Json) {
                db.changes.change(Change::Movie);
            }
        }

        if let Some((format, watched)) = format::load_array::<Watched>(&paths.watched)? {
            for w in watched {
                db.watched.insert(w);
            }

            if matches!(format, format::Format::Json) {
                db.changes.change(Change::Watched);
            }
        }

        if let Some((format, pending)) = format::load_array::<Pending>(&paths.pending)? {
            db.pending.extend(pending);

            if matches!(format, format::Format::Json) {
                db.changes.change(Change::Pending);
            }
        }

        if let Some(episodes) = format::load_directory::<_, SeriesId, Episode>(&paths.episodes)? {
            for (id, _format, mut episodes) in episodes {
                db.episodes.insert(id, episodes);
            }
        }

        if let Some(seasons) = format::load_directory::<_, SeriesId, Season>(&paths.seasons)? {
            for (id, _format, mut seasons) in seasons {
                db.seasons.insert(id, seasons);
            }
        }

        Ok(db)
    }

    /// Save any pending changes.
    pub(crate) fn save_changes(
        &mut self,
        paths: &Arc<paths::Paths>,
        do_not_save: bool,
    ) -> impl Future<Output = Result<()>> {
        let changes = std::mem::take(&mut self.changes);

        let config = changes
            .set
            .contains(Change::Config)
            .then(|| self.config.clone());

        let sync = changes
            .set
            .contains(Change::Sync)
            .then(|| self.sync.export());

        let watched = changes
            .set
            .contains(Change::Watched)
            .then(|| self.watched.export(&self.episodes));

        let pending = changes
            .set
            .contains(Change::Pending)
            .then(|| self.pending.export());

        let series = changes
            .set
            .contains(Change::Series)
            .then(|| self.series.export());

        let movies = changes
            .set
            .contains(Change::Movie)
            .then(|| self.movies.export());

        let remove_series = changes.remove_series;
        let mut add_series = Vec::with_capacity(changes.add_series.len());

        for id in changes.add_series {
            let episodes = self.episodes.by_series(&id);
            let seasons = self.seasons.by_series(&id);
            add_series.push((id, episodes.export(), seasons.export()));
        }

        let _ = changes.remove_movies;
        let _ = changes.add_movies;

        let remotes = if changes.set.contains(Change::Remotes) {
            Some(self.remotes.export())
        } else {
            None
        };

        let paths = paths.clone();
        let changes = changes.set;

        let future = async move {
            if do_not_save {
                return Ok(());
            }

            tracing::info!(?changes, "Saving database");

            let guard = paths.lock.lock().await;

            if let Some(config) = config {
                format::save_pretty("config", &paths.config, config).await?;
            }

            if let Some(sync) = sync {
                format::save_array("sync", &paths.sync, sync).await?;
            }

            if let Some(series) = series {
                format::save_array("series", &paths.series, series)
                    .await
                    .context("series")?;
            }

            if let Some(movies) = movies {
                format::save_array("movies", &paths.movies, movies)
                    .await
                    .context("movies")?;
            }

            if let Some(watched) = watched {
                format::save_array("watched", &paths.watched, watched)
                    .await
                    .context("watched")?;
            }

            if let Some(pending) = pending {
                format::save_array("pending", &paths.pending, pending)
                    .await
                    .context("pending")?;
            }

            if let Some(remotes) = remotes {
                format::save_array("remotes", &paths.remotes, remotes).await?;
            }

            for series_id in remove_series {
                let episodes_path = paths.episodes.join(format!("{series_id}"));
                let seasons_path = paths.seasons.join(format!("{series_id}"));
                let a = remove_all("episodes", episodes_path.all());
                let b = remove_all("seasons", seasons_path.all());
                let _ = tokio::try_join!(a, b)?;
            }

            for (series_id, episodes, seasons) in add_series {
                let episodes_path = paths.episodes.join(format!("{series_id}"));
                let seasons_path = paths.seasons.join(format!("{series_id}"));
                let a = format::save_array("episodes", &episodes_path, episodes);
                let b = format::save_array("seasons", &seasons_path, seasons);
                let _ = tokio::try_join!(a, b)?;
            }

            drop(guard);
            Ok(())
        };

        future.in_current_span()
    }
}

#[derive(Debug, Clone, Copy, fixed_map::Key)]
pub(crate) enum Change {
    // Configuration file has changed.
    Config,
    // Synchronization change.
    Sync,
    // Watched list has changed.
    Watched,
    // Pending list has changed.
    Pending,
    // Series list has changed.
    Series,
    // Movies have changed.
    Movie,
    // Remotes list has changed.
    Remotes,
    // Schedule changed.
    Schedule,
}

#[derive(Default)]
pub(crate) struct Changes {
    // Set of changes to apply to database.
    set: fixed_map::Set<Change>,
    // Series removed.
    remove_series: HashSet<SeriesId>,
    // Series added.
    add_series: HashSet<SeriesId>,
    // Movie removed.
    remove_movies: HashSet<MovieId>,
    // Movie added.
    add_movies: HashSet<MovieId>,
}

impl Changes {
    /// Mark a change.
    pub(crate) fn change(&mut self, change: Change) {
        self.set.insert(change);
    }

    /// Test if we contain the given change.
    pub(crate) fn contains(&self, change: Change) -> bool {
        self.set.contains(change)
    }

    #[inline]
    pub(crate) fn has_changes(&self) -> bool {
        if !self.set.is_empty() {
            return true;
        }

        if !self.remove_series.is_empty() || !self.add_series.is_empty() {
            return true;
        }

        if !self.remove_movies.is_empty() && !self.add_movies.is_empty() {
            return true;
        }

        false
    }

    /// Mark a series as added.
    pub(crate) fn add_series(&mut self, id: &SeriesId) {
        self.set.insert(Change::Series);
        self.remove_series.remove(id);
        self.add_series.insert(*id);
    }

    /// Mark a movie as added.
    pub(crate) fn add_movie(&mut self, id: &MovieId) {
        self.set.insert(Change::Movie);
        self.remove_movies.remove(id);
        self.add_movies.insert(*id);
    }

    /// Marker a series for removal.
    pub(crate) fn remove_series(&mut self, id: &SeriesId) {
        self.set.insert(Change::Series);
        self.add_series.remove(id);
        self.remove_series.insert(*id);
    }

    /// Marker a movie for removal.
    pub(crate) fn remove_movie(&mut self, id: &MovieId) {
        self.set.insert(Change::Movie);
        self.add_movies.remove(id);
        self.remove_movies.insert(*id);
    }
}

/// Remove the given file.
async fn remove_all<const N: usize>(what: &'static str, paths: [&Path; N]) -> Result<()> {
    for path in paths {
        tracing::trace!("{what}: removing: {}", path.display());

        match tokio::fs::remove_file(path).await {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }

    Ok(())
}
