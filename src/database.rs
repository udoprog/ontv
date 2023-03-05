mod format;
mod pending;
mod remotes;
mod series;
mod sync;
mod watched;

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::path::Path;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};

use crate::model::{Config, Episode, Pending, RemoteId, Season, Series, SeriesId, Watched};
use crate::queue::Queue;
use crate::service::Paths;

#[derive(Default)]
pub(crate) struct Database {
    /// Application configuration.
    pub(crate) config: Config,
    /// Remotes database.
    pub(crate) remotes: remotes::Database,
    /// Series database.
    pub(crate) series: series::Database,
    /// Episodes collection.
    pub(crate) episodes: HashMap<SeriesId, Vec<Episode>>,
    /// Seasons collection.
    pub(crate) seasons: HashMap<SeriesId, Vec<Season>>,
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
    pub(crate) fn load(paths: &Paths) -> Result<Self> {
        let mut db = Self::default();

        if let Some(config) =
            load_config(&paths.config).with_context(|| anyhow!("{}", paths.config.display()))?
        {
            db.config = config;
        }

        if let Some((_format, remotes)) = format::load_array::<_, RemoteId>(&paths.remotes)? {
            for remote_id in remotes {
                match remote_id {
                    RemoteId::Series { uuid, remotes } => {
                        for remote_id in remotes {
                            db.remotes.insert_series(remote_id, uuid);
                        }
                    }
                    RemoteId::Episode { uuid, remotes } => {
                        for remote_id in remotes {
                            db.remotes.insert_episode(remote_id, uuid);
                        }
                    }
                }
            }
        }

        if let Some((_format, syncs)) = format::load_array::<_, sync::Export>(&paths.sync)? {
            for sync in syncs {
                db.sync.import_push(sync);
            }
        }

        if let Some((index, _format, series)) =
            format::load_array_fallback::<_, Series, 2>([&paths.series_json, &paths.series_toml])?
        {
            for mut s in series {
                if let Some(remote_id) = &s.remote_id {
                    if let Some(etag) = s.compat_last_etag.take() {
                        if db.sync.update_last_etag(&s.id, remote_id, etag) {
                            db.changes.change(Change::Sync);
                        }
                    }

                    let last_modified = s.compat_last_modified.take();

                    if let Some(last_modified) = &last_modified {
                        if db
                            .sync
                            .update_last_modified(&s.id, remote_id, Some(last_modified))
                        {
                            db.changes.change(Change::Sync);
                        }
                    }
                }

                for (remote_id, last_sync) in std::mem::take(&mut s.compat_last_sync) {
                    if db.sync.import_last_sync(&s.id, &remote_id, &last_sync) {
                        db.changes.change(Change::Sync);
                    }
                }

                db.series.insert(s);
            }

            if index == 0 {
                db.changes.change(Change::Series);
            }
        }

        if let Some((_format, watched)) = format::load_array::<_, Watched>(&paths.watched)? {
            for w in watched {
                db.watched.insert(w);
            }
        }

        if let Some((_format, pending)) = format::load_array::<_, Pending>(&paths.pending)? {
            db.pending.extend(pending);
        }

        if let Some(episodes) = format::load_directory::<_, SeriesId, Episode>(&paths.episodes)? {
            for (id, _format, episodes) in episodes {
                db.episodes.insert(id, episodes);
            }
        }

        if let Some(seasons) = format::load_directory(&paths.seasons)? {
            for (id, _format, seasons) in seasons {
                db.seasons.insert(id, seasons);
            }
        }

        Ok(db)
    }

    /// Save any pending changes.
    pub(crate) fn save_changes(
        &mut self,
        paths: &Arc<Paths>,
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
            .then(|| self.watched.export());

        let pending = changes
            .set
            .contains(Change::Pending)
            .then(|| self.pending.export());

        let series = changes
            .set
            .contains(Change::Series)
            .then(|| self.series.export());

        let remove_series = changes.remove;
        let mut add_series = Vec::with_capacity(changes.add.len());

        for id in changes.add {
            let Some(episodes) = self.episodes.get(&id) else {
                continue;
            };

            let Some(seasons) = self.seasons.get(&id) else {
                continue;
            };

            add_series.push((id, episodes.clone(), seasons.clone()));
        }

        let remotes = if changes.set.contains(Change::Remotes) {
            Some(self.remotes.export())
        } else {
            None
        };

        let paths = paths.clone();

        async move {
            if do_not_save {
                return Ok(());
            }

            let guard = paths.lock.lock().await;

            if let Some(config) = config {
                format::save_pretty("config", &paths.config, config).await?;
            }

            if let Some(sync) = sync {
                format::save_array("sync", &paths.sync, sync).await?;
            }

            if let Some(series) = series {
                format::save_array_fallback(
                    "series",
                    [&paths.series_json, &paths.series_toml],
                    series,
                )
                .await?;
            }

            if let Some(watched) = watched {
                format::save_array("watched", &paths.watched, watched).await?;
            }

            if let Some(pending) = pending {
                format::save_array("pending", &paths.pending, pending).await?;
            }

            if let Some(remotes) = remotes {
                format::save_array("remotes", &paths.remotes, remotes).await?;
            }

            for series_id in remove_series {
                let episodes = paths.episodes.join(format!("{series_id}.json"));
                let seasons = paths.seasons.join(format!("{series_id}.json"));
                let a = remove_file("episodes", &episodes);
                let b = remove_file("episodes", &seasons);
                let _ = tokio::try_join!(a, b)?;
            }

            for (series_id, episodes, seasons) in add_series {
                let episodes_path = paths.episodes.join(format!("{series_id}.json"));
                let seasons_path = paths.seasons.join(format!("{series_id}.json"));
                let a = format::save_array("episodes", &episodes_path, episodes);
                let b = format::save_array("seasons", &seasons_path, seasons);
                let _ = tokio::try_join!(a, b)?;
            }

            drop(guard);
            Ok(())
        }
    }
}

#[derive(Clone, Copy, fixed_map::Key)]
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
    // Remotes list has changed.
    Remotes,
    // Task queue has changed.
    Queue,
    // Schedule changed.
    Schedule,
}

#[derive(Default)]
pub(crate) struct Changes {
    // Set of changes to apply to database.
    set: fixed_map::Set<Change>,
    // Series removed.
    remove: HashSet<SeriesId>,
    // Series added.
    add: HashSet<SeriesId>,
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
        !self.set.is_empty() || !self.remove.is_empty() || !self.add.is_empty()
    }

    /// Mark a series as added.
    pub(crate) fn add_series(&mut self, id: &SeriesId) {
        self.set.insert(Change::Series);
        self.remove.remove(id);
        self.add.insert(*id);
    }

    /// Marker a series for removal.
    pub(crate) fn remove_series(&mut self, id: &SeriesId) {
        self.set.insert(Change::Series);
        self.add.remove(id);
        self.remove.insert(*id);
    }
}

/// Load configuration file.
pub(crate) fn load_config(path: &Path) -> Result<Option<Config>> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };

    Ok(serde_json::from_slice(&bytes)?)
}

/// Remove the given file.
async fn remove_file(what: &'static str, path: &Path) -> Result<()> {
    tracing::trace!("{what}: removing: {}", path.display());
    let _ = tokio::fs::remove_file(path).await;
    Ok(())
}
