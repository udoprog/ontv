use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;
use std::future::Future;
use std::path::Path;
use std::sync::Arc;

use anyhow::{anyhow, Context, Error, Result};
use chrono::{DateTime, Utc};
use iced_native::image::Handle;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::message::Message;
use crate::model::{
    Episode, Image, RemoteEpisodeId, RemoteId, RemoteSeriesId, Season, SeasonNumber, Series,
    Watched,
};
use crate::page::settings::Settings;
use crate::thetvdb::Client;

/// Data encapsulating a newly added series.
#[derive(Clone)]
pub(crate) struct NewSeries {
    series: Series,
    episodes: Vec<Episode>,
    seasons: Vec<Season>,
    refresh_pending: bool,
}

impl fmt::Debug for NewSeries {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NewSeries").finish_non_exhaustive()
    }
}

/// A pending thing to watch.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub(crate) struct Pending {
    pub(crate) series: Uuid,
    pub(crate) episode: Uuid,
    pub(crate) timestamp: DateTime<Utc>,
}

/// A pending thing to watch.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PendingRef<'a> {
    pub(crate) series: &'a Series,
    pub(crate) episode: &'a Episode,
}

#[derive(Default)]
struct Changes {
    watched: bool,
    pending: bool,
    series: bool,
    remotes: bool,
    remove: HashSet<Uuid>,
    add: HashSet<Uuid>,
}

impl Changes {
    fn has_changes(&self) -> bool {
        self.watched
            || self.pending
            || self.series
            || self.remotes
            || !self.remove.is_empty()
            || !self.add.is_empty()
    }
}

#[derive(Default)]
struct Database {
    remote_series: BTreeMap<RemoteSeriesId, Uuid>,
    remote_episodes: BTreeMap<RemoteEpisodeId, Uuid>,
    series: BTreeMap<Uuid, Series>,
    episodes: HashMap<Uuid, Vec<Episode>>,
    seasons: HashMap<Uuid, Vec<Season>>,
    watched: Vec<Watched>,
    /// Temporary set of watched episodes and its corresponding watch count.
    watch_counts: HashMap<Uuid, usize>,
    /// Ordered list of things to watch.
    pending: Vec<Pending>,
    /// Keeping track of changes to be saved.
    changes: Changes,
}

struct Paths {
    /// Mutex to avoid clobbering the filesystem with multiple concurrent writes - but only from the same application.
    lock: tokio::sync::Mutex<()>,
    /// Path to configuration file.
    config: Box<Path>,
    /// Path where remote mappings are stored.
    remotes: Box<Path>,
    /// Images configuration directory.
    images: Box<Path>,
    /// Path where series are stored.
    series: Box<Path>,
    /// Watch history.
    watched: Box<Path>,
    /// Pending history.
    pending: Box<Path>,
    /// Path where episodes are stored.
    episodes: Box<Path>,
    /// Path where seasons are stored.
    seasons: Box<Path>,
}

/// Background service taking care of all state handling.
pub struct Service {
    paths: Arc<Paths>,
    /// Service database.
    db: Database,
    /// Shared client.
    pub(crate) client: Client,
}

impl Service {
    /// Construct and setup in-memory state of
    pub(crate) fn new() -> Result<(Self, Settings)> {
        let dirs = directories_next::ProjectDirs::from("se.tedro", "setbac", "OnTV")
            .context("missing project dirs")?;

        let paths = Paths {
            lock: tokio::sync::Mutex::new(()),
            config: dirs.config_dir().join("config.json").into(),
            remotes: dirs.config_dir().join("remotes.json").into(),
            series: dirs.config_dir().join("series.json").into(),
            watched: dirs.config_dir().join("watched.json").into(),
            pending: dirs.config_dir().join("pending.json").into(),
            episodes: dirs.config_dir().join("episodes").into(),
            seasons: dirs.config_dir().join("seasons").into(),
            images: dirs.cache_dir().join("images").into(),
        };

        let db = load_database(&paths)?;

        let settings = match load_config(&paths.config)? {
            Some(settings) => settings,
            None => Default::default(),
        };

        let client = Client::new(&settings.thetvdb_legacy_apikey);

        let this = Self {
            paths: Arc::new(paths),
            db,
            client,
        };

        Ok((this, settings))
    }

    /// Get a single series.
    pub(crate) fn series(&self, id: Uuid) -> Option<&Series> {
        self.db.series.get(&id)
    }

    /// Get list of series.
    pub(crate) fn all_series(&self) -> impl Iterator<Item = &Series> {
        self.db.series.values()
    }

    /// Iterator over available episodes.
    pub(crate) fn episodes(&self, id: Uuid) -> impl Iterator<Item = &Episode> {
        self.db.episodes.get(&id).into_iter().flatten()
    }

    /// Iterator over available seasons.
    pub(crate) fn seasons(&self, id: Uuid) -> impl Iterator<Item = &Season> {
        self.db.seasons.get(&id).into_iter().flatten()
    }

    /// Get watch history.
    pub(crate) fn watched(&self) -> impl Iterator<Item = &Watched> {
        self.db.watched.iter()
    }

    /// Test if episode is watched.
    pub(crate) fn watch_count(&self, episode_id: Uuid) -> usize {
        self.db
            .watch_counts
            .get(&episode_id)
            .copied()
            .unwrap_or_default()
    }

    /// Get the pending episode for the given series.
    pub(crate) fn get_pending(&self, series_id: Uuid) -> Option<&Pending> {
        self.db
            .pending
            .iter()
            .filter(|p| p.series == series_id)
            .next()
    }

    /// Return list of pending episodes.
    pub(crate) fn pending(&self) -> impl DoubleEndedIterator<Item = PendingRef<'_>> {
        self.db.pending.iter().flat_map(|p| {
            let series = self.db.series.get(&p.series)?;
            let episodes = self.db.episodes.get(&p.series)?;
            let episode = episodes.iter().find(|e| e.id == p.episode)?;

            Some(PendingRef { series, episode })
        })
    }

    /// Test if we have changes.
    pub(crate) fn has_changes(&self) -> bool {
        self.db.changes.has_changes()
    }

    /// Mark an episode as watched at the given timestamp.
    pub(crate) fn watch_remaining_season(
        &mut self,
        series: Uuid,
        season: SeasonNumber,
        timestamp: DateTime<Utc>,
    ) {
        let mut last = None;

        for episode in self
            .db
            .episodes
            .get(&series)
            .into_iter()
            .flatten()
            .filter(|e| e.season == season)
        {
            if self.watch_count(episode.id) > 0 {
                continue;
            }

            if !episode.has_aired(&timestamp) {
                continue;
            }

            self.db.watched.push(Watched {
                id: Uuid::new_v4(),
                series,
                episode: episode.id,
                timestamp,
            });

            *self.db.watch_counts.entry(episode.id).or_default() += 1;
            self.db.changes.watched = true;
            last = Some(episode.id);
        }

        let Some(last) = last else {
            return;
        };

        self.setup_pending(series, Some(last));
    }

    /// Mark an episode as watched at the given timestamp.
    pub(crate) fn watch(&mut self, series: Uuid, episode: Uuid, timestamp: DateTime<Utc>) {
        self.db.watched.push(Watched {
            id: Uuid::new_v4(),
            series,
            episode,
            timestamp,
        });

        *self.db.watch_counts.entry(episode).or_default() += 1;
        self.db.changes.watched = true;
        self.setup_pending(series, Some(episode))
    }

    /// Skip an episode.
    pub(crate) fn skip(&mut self, series_id: Uuid, episode_id: Uuid, timestamp: DateTime<Utc>) {
        let Some(episodes) = self.db.episodes.get(&series_id) else {
            return;
        };

        let mut it = episodes.iter();

        while let Some(episode) = it.next() {
            if episode.id == episode_id {
                break;
            }
        }

        let Some(episode) = it.next() else {
            return;
        };

        let mut changed = false;

        for pending in self
            .db
            .pending
            .iter_mut()
            .filter(|p| p.episode == episode_id)
        {
            pending.episode = episode.id;
            pending.timestamp = timestamp;
            changed = true;
            break;
        }

        self.db.changes.pending |= changed;
    }

    /// Select the next pending episode to use for a show.
    pub(crate) fn select_pending(
        &mut self,
        series_id: Uuid,
        episode_id: Uuid,
        timestamp: DateTime<Utc>,
    ) {
        self.db.changes.pending = true;

        // Try to modify in-place.
        if let Some(pending) = self
            .db
            .pending
            .iter_mut()
            .filter(|p| p.series == series_id)
            .next()
        {
            pending.episode = episode_id;
            pending.timestamp = timestamp;
        } else {
            self.db.pending.push(Pending {
                series: series_id,
                episode: episode_id,
                timestamp,
            });
        }

        self.db
            .pending
            .sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
    }

    /// Remove all watches of the given episode.
    pub(crate) fn remove_episode_watches(
        &mut self,
        series: Uuid,
        episode: Uuid,
        timestamp: DateTime<Utc>,
    ) {
        self.db.watched.retain(|w| w.episode != episode);
        let _ = self.db.watch_counts.remove(&episode);
        self.db.pending.retain(|p| p.series != series);

        self.db.pending.push(Pending {
            series,
            episode,
            timestamp,
        });

        self.db.changes.watched = true;
        self.db.changes.pending = true;
    }

    /// Remove all watches of the given episode.
    pub(crate) fn remove_season_watches(
        &mut self,
        series: Uuid,
        season: SeasonNumber,
        timestamp: DateTime<Utc>,
    ) {
        let mut removed = Vec::new();

        self.db.watched.retain(|w| {
            if w.series != series {
                return true;
            };

            let Some(episodes) = self.db.episodes.get(&w.series) else {
                return true;
            };

            let Some(episode) = episodes.iter().find(|e| e.id == w.episode) else {
                return true;
            };

            if episode.season == season {
                removed.push(episode.id);
                false
            } else {
                true
            }
        });

        for id in removed {
            let _ = self.db.watch_counts.remove(&id);
        }

        self.db.changes.watched = true;
        self.db.pending.retain(|p| p.series != series);
        self.db.changes.pending = true;

        let Some(episodes) = self.db.episodes.get(&series) else {
            return;
        };

        // Find the first episode matching the given season and make that the
        // pending episode.
        if let Some(episode) = episodes.iter().find(|e| e.season == season) {
            self.db.pending.push(Pending {
                series,
                episode: episode.id,
                timestamp,
            });
        }
    }

    /// Set up next pending episode.
    fn setup_pending(&mut self, series: Uuid, episode: Option<Uuid>) {
        // Remove any pending episodes for the given series.
        self.db.pending.retain(|p| p.series != series);
        self.db.changes.pending = true;
        let now = Utc::now();
        self.populate_pending(&now, series, episode);
    }

    /// Save changes made.
    pub(crate) fn save_changes(&mut self) -> impl Future<Output = Result<()>> {
        let changes = std::mem::take(&mut self.db.changes);

        let watched = changes.watched.then(|| self.db.watched.clone());
        let pending = changes.pending.then(|| self.db.pending.clone());
        let series = changes.series.then(|| self.db.series.clone());
        let remove_series = changes.remove;
        let mut add_series = Vec::with_capacity(changes.add.len());

        for id in changes.add {
            let Some(episodes) = self.db.episodes.get(&id) else {
                continue;
            };

            let Some(seasons) = self.db.seasons.get(&id) else {
                continue;
            };

            add_series.push((id, episodes.clone(), seasons.clone()));
        }

        let remotes = if changes.remotes {
            let mut remotes =
                Vec::with_capacity(&self.db.remote_series.len() + self.db.remote_episodes.len());

            for (id, series_id) in &self.db.remote_series {
                remotes.push((series_id.clone(), RemoteId::Series { id: *id }));
            }

            for (id, series_id) in &self.db.remote_episodes {
                remotes.push((series_id.clone(), RemoteId::Episode { id: *id }));
            }

            Some(remotes)
        } else {
            None
        };

        let paths = self.paths.clone();

        async move {
            let guard = paths.lock.lock().await;

            if let Some(series) = series {
                save_array("series", &paths.series, series.values()).await?;
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

            for series_id in remove_series {
                let episodes = paths.episodes.join(format!("{}.json", series_id));
                let seasons = paths.seasons.join(format!("{}.json", series_id));
                let a = remove_file("episodes", &episodes);
                let b = remove_file("episodes", &seasons);
                let _ = tokio::try_join!(a, b)?;
            }

            for (series_id, episodes, seasons) in add_series {
                let episodes_path = paths.episodes.join(format!("{}.json", series_id));
                let seasons_path = paths.seasons.join(format!("{}.json", series_id));
                let a = save_array("episodes", &episodes_path, episodes);
                let b = save_array("seasons", &seasons_path, seasons);
                let _ = tokio::try_join!(a, b)?;
            }

            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            drop(guard);
            Ok(())
        }
    }

    /// Ensure that at least one pending episode is present for the given
    /// series.
    fn populate_pending(&mut self, now: &DateTime<Utc>, series: Uuid, last: Option<Uuid>) {
        // Populate the next pending episode.
        let Some(episodes) = self.db.episodes.get(&series) else {
            return;
        };

        let mut it = episodes.iter().peekable();

        if let Some(last) = last {
            // Find the first episode which is after the last episode indicated.
            while let Some(e) = it.next() {
                if e.id == last {
                    break;
                }
            }
        } else {
            // Find the first episode which is *not* in our watch history.
            while let Some(e) = it.peek() {
                if self.watch_count(e.id) == 0 {
                    break;
                }

                it.next();
            }
        }

        // Mark the first episode (that has aired).
        while let Some(e) = it.next() {
            if !e.has_aired(now) {
                break;
            }

            // Mark the next episode in the show as pending.
            self.db.pending.push(Pending {
                series,
                episode: e.id,
                timestamp: *now,
            });

            break;
        }

        self.db
            .pending
            .sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
    }

    /// Load configuration file.
    pub(crate) fn save_config(&mut self, settings: Settings) -> impl Future<Output = Message> {
        self.client.set_api_key(&settings.thetvdb_legacy_apikey);
        let paths = self.paths.clone();

        async move {
            match save_config(&paths.config, &settings).await {
                Ok(()) => Message::SavedConfig,
                Err(e) => Message::error(e),
            }
        }
    }

    /// Check if series is tracked.
    pub(crate) fn get_series_by_remote(&self, id: RemoteSeriesId) -> Option<&Series> {
        let id = self.db.remote_series.get(&id)?;
        self.db.series.get(id)
    }

    /// Refresh series data.
    pub(crate) fn refresh_series(
        &mut self,
        series_id: Uuid,
    ) -> Option<impl Future<Output = Result<NewSeries>>> {
        let series = self.db.series.get(&series_id)?;
        let remote_id = *series.remote_ids.iter().next()?;
        Some(self.download_series(remote_id, series_id, false))
    }

    /// Remove the given series by ID.
    pub(crate) fn remove_series(&mut self, series_id: Uuid) {
        let _ = self.db.series.remove(&series_id);
        let _ = self.db.episodes.remove(&series_id);
        let _ = self.db.seasons.remove(&series_id);
        self.db.changes.series = true;
        self.db.changes.add.remove(&series_id);
        self.db.changes.remove.insert(series_id);
    }

    /// Enable tracking of the series with the given id.
    pub(crate) fn download_series_by_remote(
        &self,
        id: RemoteSeriesId,
    ) -> impl Future<Output = Result<NewSeries>> {
        let series_id = self
            .db
            .remote_series
            .iter()
            .find(|(remote_id, _)| **remote_id == id)
            .map(|(_, &id)| id)
            .unwrap_or_else(Uuid::new_v4);

        self.download_series(id, series_id, true)
    }

    /// Download series using a remote identifier.
    fn download_series(
        &self,
        remote_id: RemoteSeriesId,
        series_id: Uuid,
        refresh_pending: bool,
    ) -> impl Future<Output = Result<NewSeries>> {
        let client = self.client.clone();
        let remote_episodes = self.db.remote_episodes.clone();

        async move {
            let lookup = |q| {
                remote_episodes
                    .iter()
                    .find(|(remote_id, _)| **remote_id == q)
                    .map(|(_, &id)| id)
                    .unwrap_or_else(Uuid::new_v4)
            };

            let (series, episodes, seasons) = match remote_id {
                RemoteSeriesId::TheTvDb { id } => {
                    let series = client.series(id, series_id);
                    let episodes = client
                        .series_episodes(id, move |id| lookup(RemoteEpisodeId::TheTvDb { id }));
                    let (series, episodes) = tokio::try_join!(series, episodes)?;
                    let seasons = episodes_into_seasons(&episodes);
                    (series, episodes, seasons)
                }
            };

            let data = NewSeries {
                series,
                episodes,
                seasons,
                refresh_pending,
            };

            Ok::<_, Error>(data)
        }
    }

    /// If the series is already loaded in the local database, simply mark it as tracked.
    pub(crate) fn set_tracked_by_remote(&mut self, id: RemoteSeriesId) -> bool {
        let Some(&id) = self.db.remote_series.get(&id) else {
            return false;
        };

        self.track(id)
    }

    /// Set the given show as tracked.
    pub(crate) fn track(&mut self, series_id: Uuid) -> bool {
        let Some(series) = self.db.series.get_mut(&series_id) else {
            return false;
        };

        series.tracked = true;
        self.db.changes.series = true;
        self.setup_pending(series_id, None);
        true
    }

    /// Disable tracking of the series with the given id.
    pub(crate) fn untrack(&mut self, series_id: Uuid) {
        if let Some(s) = self.db.series.get_mut(&series_id) {
            s.tracked = false;
            self.db.changes.series = true;
        }

        self.db.pending.retain(|p| p.series != series_id);
        self.db.changes.pending = true;
    }

    /// Insert a new tracked song.
    pub(crate) fn insert_new_series(&mut self, data: NewSeries) {
        let series_id = data.series.id;

        for remote_id in &data.series.remote_ids {
            self.db.remote_series.insert(*remote_id, series_id);
        }

        for episode in &data.episodes {
            for remote_id in &episode.remote_ids {
                self.db.remote_episodes.insert(*remote_id, episode.id);
            }
        }

        self.db.episodes.insert(series_id, data.episodes.clone());

        self.db.seasons.insert(series_id, data.seasons.clone());
        self.db.series.insert(series_id, data.series);

        if data.refresh_pending {
            // Remove any pending episodes for the given series.
            self.db.pending.retain(|p| p.series != series_id);
            let now = Utc::now();
            self.populate_pending(&now, series_id, None);
            self.db.changes.pending = true;
        }

        self.db.changes.series = true;
        self.db.changes.remotes = true;
        self.db.changes.remove.remove(&series_id);
        self.db.changes.add.insert(series_id);
    }

    /// Ensure that a collection of the given image ids are loaded.
    pub(crate) fn load_image(
        &self,
        id: Image,
    ) -> impl Future<Output = Result<Vec<(Image, Handle)>>> {
        let client = self.client.clone();
        let paths = self.paths.clone();
        cache_images(client, paths, [id])
    }
}

/// Load configuration file.
pub(crate) fn load_config(path: &Path) -> Result<Option<Settings>> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };

    Ok(serde_json::from_slice(&bytes)?)
}

/// Save configuration file.
pub(crate) async fn save_config(path: &Path, state: &Settings) -> Result<()> {
    use tokio::fs;

    let bytes = serde_json::to_vec_pretty(state)?;

    if let Some(d) = path.parent() {
        if !matches!(fs::metadata(d).await, Ok(m) if m.is_dir()) {
            log::info!("creating directory: {}", d.display());
            fs::create_dir_all(d).await?;
        }
    }

    fs::write(path, bytes).await?;
    Ok(())
}

/// Ensure that the given image IDs are in the in-memory and filesystem image
/// caches.
async fn cache_images<I>(client: Client, paths: Arc<Paths>, ids: I) -> Result<Vec<(Image, Handle)>>
where
    I: IntoIterator<Item = Image>,
{
    use tokio::fs;

    let mut output = Vec::new();

    for id in ids {
        let hash = id.hash();
        let cache_path = paths.images.join(format!("{:032x}.{}", hash, id.format()));

        let data = if matches!(fs::metadata(&cache_path).await, Ok(m) if m.is_file()) {
            log::debug!("reading image from cache: {id}: {}", cache_path.display());
            fs::read(&cache_path).await?
        } else {
            log::debug!("downloading: {id}: {}", cache_path.display());
            let data = client.get_image_data(&id).await?;

            if let Some(parent) = cache_path.parent() {
                if !matches!(fs::metadata(parent).await, Ok(m) if m.is_dir()) {
                    log::debug!("creating image cache directory: {}", parent.display());
                    fs::create_dir_all(parent).await?;
                }
            }

            fs::write(&cache_path, &data).await?;
            data
        };

        log::debug!("loaded: {id} ({} bytes)", data.len());
        let handle = Handle::from_memory(data);
        output.push((id, handle));
    }

    Ok(output)
}

/// Try to load initial state.
fn load_database(paths: &Paths) -> Result<Database> {
    let mut db = Database::default();

    if let Some(remotes) = load_array::<(Uuid, RemoteId)>(&paths.remotes)? {
        for (uuid, remote_id) in remotes {
            match remote_id {
                RemoteId::Series { id } => {
                    db.remote_series.insert(id, uuid);
                }
                RemoteId::Episode { id } => {
                    db.remote_episodes.insert(id, uuid);
                }
            }
        }
    }

    if let Some(series) = load_series(&paths.series)? {
        for s in series {
            for &id in &s.remote_ids {
                db.remote_series.insert(id, s.id);
            }

            db.series.insert(s.id, s);
        }
    }

    if let Some(watched) = load_array::<Watched>(&paths.watched)? {
        for w in &watched {
            *db.watch_counts.entry(w.episode).or_default() += 1;
        }

        db.watched = watched;
    }

    if let Some(pending) = load_array(&paths.pending)? {
        db.pending = pending;
    }

    if let Some(episodes) = load_directory(&paths.episodes)? {
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
    let _ = tokio::fs::remove_file(path);
    Ok(())
}

/// Save series to the given path.
async fn save_array<I>(what: &'static str, path: &Path, data: I) -> Result<()>
where
    I: IntoIterator,
    I::Item: Serialize,
{
    use tokio::fs;
    use tokio::io::AsyncWriteExt;

    log::debug!("saving {what}: {}", path.display());

    if let Some(d) = path.parent() {
        if !matches!(fs::metadata(d).await, Ok(m) if m.is_dir()) {
            fs::create_dir_all(d).await?;
        }
    }

    let mut f = fs::File::create(path).await?;
    let mut line = Vec::new();

    for episode in data {
        line.clear();
        serde_json::to_writer(&mut line, &episode)?;
        line.push(b'\n');
        f.write_all(&line).await?;
    }

    Ok(())
}

/// Load all episodes found on the given paths.
fn load_directory<T>(path: &Path) -> Result<Option<Vec<(Uuid, Vec<T>)>>>
where
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
    let f = match std::fs::File::open(&path) {
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

        output.push(serde_json::from_str(&line)?);
    }

    Ok(output)
}

/// Helper to build seasons out of known episodes.
fn episodes_into_seasons(episodes: &[Episode]) -> Vec<Season> {
    let mut map = BTreeMap::new();

    for e in episodes {
        map.entry(e.season)
            .or_insert_with(|| Season { number: e.season });
    }

    map.into_iter().map(|(_, value)| value).collect()
}
