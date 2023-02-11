mod pending;
mod remotes;
mod series;
mod sync;
mod watched;

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::future::Future;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::{anyhow, Context, Error, Result};
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::model::{Config, Episode, Pending, RemoteId, Season, Series, SeriesId, Task, Watched};
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

        if let Some(remotes) = load_array::<RemoteId>(&paths.remotes)? {
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

        if let Some(syncs) = load_array::<sync::Export>(&paths.sync)? {
            for sync in syncs {
                db.sync.import_push(sync);
            }
        }

        if let Some(tasks) = load_array::<Task>(&paths.queue)? {
            for task in tasks {
                db.tasks.import_push(task);
            }

            db.tasks.sort();
        }

        if let Some(series) = load_series(&paths.series)? {
            for mut s in series {
                if let Some(etag) = s.compat_last_etag.take() {
                    if db.sync.update_last_etag(&s.id, etag) {
                        db.changes.change(Change::Sync);
                    }
                }

                let last_modified = s.compat_last_modified.take();

                if let (Some(remote_id), Some(last_modified)) = (&s.remote_id, &last_modified) {
                    if db
                        .sync
                        .update_last_modified(&s.id, remote_id, Some(last_modified))
                    {
                        db.changes.change(Change::Sync);
                    }
                }

                for (remote_id, last_sync) in std::mem::take(&mut s.compat_last_sync) {
                    if db.sync.import_last_sync(&s.id, &remote_id, &last_sync) {
                        db.changes.change(Change::Sync);
                    }
                }

                db.series.insert(s);
            }
        }

        if let Some(watched) = load_array::<Watched>(&paths.watched)? {
            for w in watched {
                db.watched.insert(w);
            }
        }

        if let Some(pending) = load_array::<Pending>(&paths.pending)? {
            db.pending.extend(pending);
        }

        if let Some(episodes) = load_directory::<SeriesId, Episode>(&paths.episodes)? {
            for (id, episodes) in episodes {
                db.episodes.insert(id, episodes);
            }
        }

        if let Some(seasons) = load_directory(&paths.seasons)? {
            for (id, seasons) in seasons {
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

        let queue = changes
            .set
            .contains(Change::Queue)
            .then(|| self.tasks.pending().cloned().collect::<Vec<_>>());

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
                save_pretty("config", &paths.config, config).await?;
            }

            if let Some(sync) = sync {
                save_array("sync", &paths.sync, sync).await?;
            }

            if let Some(series) = series {
                save_array("series", &paths.series, series).await?;
            }

            if let Some(watched) = watched {
                save_array("watched", &paths.watched, watched).await?;
            }

            if let Some(pending) = pending {
                save_array("pending", &paths.pending, pending).await?;
            }

            if let Some(remotes) = remotes {
                save_array("remotes", &paths.remotes, remotes).await?;
            }

            if let Some(queue) = queue {
                save_array("queue", &paths.queue, queue).await?;
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
                let a = save_array("episodes", &episodes_path, episodes);
                let b = save_array("seasons", &seasons_path, seasons);
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

/// Load series from the given path.
fn load_series(path: &Path) -> Result<Option<Vec<Series>>> {
    let f = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };

    Ok(Some(
        load_array_from_reader(f).with_context(|| anyhow!("{}", path.display()))?,
    ))
}

/// Remove the given file.
async fn remove_file(what: &'static str, path: &Path) -> Result<()> {
    log::trace!("{what}: removing: {}", path.display());
    let _ = tokio::fs::remove_file(path).await;
    Ok(())
}

/// Save series to the given path.
fn save_pretty<I>(what: &'static str, path: &Path, data: I) -> impl Future<Output = Result<()>>
where
    I: 'static + Send + Serialize,
{
    use std::fs;
    use std::io::Write;

    log::debug!("saving {what}: {}", path.display());

    let path = path.to_owned();

    let task = tokio::spawn(async move {
        let Some(dir) = path.parent() else {
            anyhow::bail!("{what}: missing parent directory: {}", path.display());
        };

        if !matches!(fs::metadata(dir), Ok(m) if m.is_dir()) {
            fs::create_dir_all(dir)?;
        }

        let mut f = tempfile::NamedTempFile::new_in(dir)?;

        log::trace!("writing {what}: {}", f.path().display());

        serde_json::to_writer_pretty(&mut f, &data)?;
        f.write_all(&[b'\n'])?;

        f.flush()?;
        let (_, temp_path) = f.keep()?;

        log::trace!(
            "rename {what}: {} -> {}",
            temp_path.display(),
            path.display()
        );

        fs::rename(temp_path, path)?;
        Ok(())
    });

    async move {
        let output: Result<()> = task.await?;
        output
    }
}

/// Save series to the given path.
fn save_array<I>(what: &'static str, path: &Path, data: I) -> impl Future<Output = Result<()>>
where
    I: 'static + Send + IntoIterator,
    I::Item: Serialize,
{
    use std::fs;
    use std::io::Write;

    log::trace!("saving {what}: {}", path.display());

    let path = path.to_owned();

    let task = tokio::spawn(async move {
        let Some(dir) = path.parent() else {
            anyhow::bail!("{what}: missing parent directory: {}", path.display());
        };

        if !matches!(fs::metadata(dir), Ok(m) if m.is_dir()) {
            fs::create_dir_all(dir)?;
        }

        let mut f = tempfile::NamedTempFile::new_in(dir)?;

        log::trace!("writing {what}: {}", f.path().display());

        for line in data {
            serde_json::to_writer(&mut f, &line)?;
            f.write_all(&[b'\n'])?;
        }

        f.flush()?;
        let (_, temp_path) = f.keep()?;

        log::trace!(
            "rename {what}: {} -> {}",
            temp_path.display(),
            path.display()
        );

        fs::rename(temp_path, path)?;
        Ok(())
    });

    async move {
        let output: Result<()> = task.await?;
        output
    }
}

/// Load all episodes found on the given paths.
fn load_directory<I, T>(path: &Path) -> Result<Option<Vec<(I, Vec<T>)>>>
where
    I: FromStr,
    I::Err: fmt::Display,
    T: DeserializeOwned,
{
    use std::fs;

    let d = match fs::read_dir(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };

    let mut output = Vec::new();

    for e in d {
        let e = e?;

        let m = e.metadata()?;

        if !m.is_file() {
            continue;
        }

        let path = e.path();

        if !matches!(path.extension().and_then(|e| e.to_str()), Some("json")) {
            continue;
        }

        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };

        let Ok(id) = stem.parse() else {
            continue;
        };

        let f = std::fs::File::open(&path)?;
        let array = load_array_from_reader(f).with_context(|| anyhow!("{}", path.display()))?;
        output.push((id, array));
    }

    Ok(Some(output))
}

/// Load a simple array from a file.
fn load_array<T>(path: &Path) -> Result<Option<Vec<T>>>
where
    T: DeserializeOwned,
{
    let f = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(Error::from(e)).with_context(|| anyhow!("{}", path.display())),
    };

    Ok(Some(
        load_array_from_reader(f).with_context(|| anyhow!("{}", path.display()))?,
    ))
}

/// Load an array from the given reader line-by-line.
fn load_array_from_reader<I, T>(input: I) -> Result<Vec<T>>
where
    I: std::io::Read,
    T: DeserializeOwned,
{
    use std::io::{BufRead, BufReader};

    let mut output = Vec::new();

    for line in BufReader::new(input).lines() {
        let line = line?;
        let line = line.trim();

        if line.starts_with('#') || line.is_empty() {
            continue;
        }

        output.push(serde_json::from_str(line)?);
    }

    Ok(output)
}
