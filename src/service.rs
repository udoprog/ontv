use std::collections::{BTreeMap, HashMap};
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
    Episode, Image, RemoteEpisodeId, RemoteId, RemoteSeriesId, Season, Series, Watched,
};
use crate::page::settings::Settings;
use crate::thetvdb::Client;

/// Data encapsulating a newly added series.
#[derive(Debug, Clone)]
pub(crate) struct NewSeries {
    series: Series,
    episodes: Vec<Episode>,
    seasons: Vec<Season>,
}

/// A pending thing to watch.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub(crate) struct Pending {
    series: Uuid,
    episode: Uuid,
    timestamp: DateTime<Utc>,
}

/// A pending thing to watch.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PendingRef<'a> {
    pub(crate) series: &'a Series,
    pub(crate) episode: &'a Episode,
}

#[derive(Clone, Default)]
struct Database {
    remote_series: BTreeMap<RemoteSeriesId, Uuid>,
    remote_episodes: BTreeMap<RemoteEpisodeId, Uuid>,
    series: BTreeMap<Uuid, Series>,
    episodes: HashMap<Uuid, Vec<Episode>>,
    seasons: HashMap<Uuid, Vec<Season>>,
    watched: Vec<Watched>,
    /// Ordered list of things to watch.
    pending: Vec<Pending>,
}

struct Paths {
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

    /// Return list of pending episodes.
    pub(crate) fn pending(&self) -> impl Iterator<Item = PendingRef<'_>> {
        self.db.pending.iter().flat_map(|p| {
            let series = self.db.series.get(&p.series)?;
            let episodes = self.db.episodes.get(&p.series)?;
            let episode = episodes.iter().find(|e| e.id == p.episode)?;
            Some(PendingRef { series, episode })
        })
    }

    /// Mark an episode as watched at the given timestamp.
    pub(crate) fn watch(
        &mut self,
        series: Uuid,
        episode: Uuid,
        timestamp: DateTime<Utc>,
    ) -> impl Future<Output = Result<()>> {
        self.db.watched.push(Watched {
            id: Uuid::new_v4(),
            series,
            episode,
            timestamp,
        });
        let paths = self.paths.clone();
        let watched = self.db.watched.clone();

        // Remove any pending episodes for the given series.
        self.db.pending.retain(|p| p.series != series);

        let now = Utc::now();
        self.populate_pending(&now, series, Some(episode));
        let pending = self.db.pending.clone();

        async move {
            save_array("watched", &paths.watched, watched).await?;
            save_array("pending", &paths.pending, pending).await?;
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
                if !self.db.watched.iter().any(|w| w.episode == e.id) {
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
            .sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
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

    /// Remove the given series by ID.
    pub(crate) fn remove_series(&mut self, series_id: Uuid) -> impl Future<Output = Result<()>> {
        let _ = self.db.series.remove(&series_id);
        let _ = self.db.episodes.remove(&series_id);
        let _ = self.db.seasons.remove(&series_id);

        let paths = self.paths.clone();
        let series = self.db.series.clone();

        async move {
            let episodes = paths.episodes.join(format!("{}.json", series_id));
            let seasons = paths.seasons.join(format!("{}.json", series_id));
            let a = save_array("series", &paths.series, series.values());
            let b = remove_file("episodes", &episodes);
            let c = remove_file("episodes", &seasons);
            let _ = tokio::try_join!(a, b, c)?;
            Ok(())
        }
    }

    /// Enable tracking of the series with the given id.
    pub(crate) fn add_series_by_remote(
        &self,
        id: RemoteSeriesId,
    ) -> impl Future<Output = Result<NewSeries>> {
        fn seasons(episodes: &[Episode]) -> Vec<Season> {
            let mut map = BTreeMap::new();

            for e in episodes {
                map.entry(e.season)
                    .or_insert_with(|| Season { number: e.season });
            }

            map.into_iter().map(|(_, value)| value).collect()
        }

        let client = self.client.clone();

        let new_id = self
            .db
            .remote_series
            .iter()
            .find(|(remote_id, _)| **remote_id == id)
            .map(|(_, &id)| id)
            .unwrap_or_else(Uuid::new_v4);

        let remote_episodes = self.db.remote_episodes.clone();

        async move {
            let lookup = |q| {
                remote_episodes
                    .iter()
                    .find(|(remote_id, _)| **remote_id == q)
                    .map(|(_, &id)| id)
                    .unwrap_or_else(Uuid::new_v4)
            };

            let (series, episodes, seasons) = match id {
                RemoteSeriesId::TheTvDb { id } => {
                    let series = client.series(id, new_id);
                    let episodes = client
                        .series_episodes(id, move |id| lookup(RemoteEpisodeId::TheTvDb { id }));
                    let (series, episodes) = tokio::try_join!(series, episodes)?;
                    let seasons = seasons(&episodes);
                    (series, episodes, seasons)
                }
            };

            let data = NewSeries {
                series,
                episodes,
                seasons,
            };

            Ok::<_, Error>(data)
        }
    }

    /// If the series is already loaded in the local database, simply mark it as tracked.
    pub(crate) fn set_tracked_by_remote(
        &mut self,
        id: RemoteSeriesId,
    ) -> Option<impl Future<Output = Result<()>>> {
        let id = *self.db.remote_series.get(&id)?;
        self.set_tracked(id)
    }

    /// Set the given show as tracked.
    pub(crate) fn set_tracked(&mut self, id: Uuid) -> Option<impl Future<Output = Result<()>>> {
        let s = self.db.series.get_mut(&id)?;
        s.tracked = true;

        let paths = self.paths.clone();
        let series = self.db.series.clone();

        Some(
            async move { save_array("series", &paths.series, series.into_iter().map(|d| d.1)).await },
        )
    }

    /// Disable tracking of the series with the given id.
    pub(crate) fn untrack(&mut self, id: Uuid) -> Option<impl Future<Output = Result<()>>> {
        let s = self.db.series.get_mut(&id)?;
        s.tracked = false;

        let paths = self.paths.clone();
        let series = self.db.series.clone();

        Some(
            async move { save_array("series", &paths.series, series.into_iter().map(|d| d.1)).await },
        )
    }

    /// Insert a new tracked song.
    pub(crate) fn insert_new_series(
        &mut self,
        data: NewSeries,
    ) -> Option<impl Future<Output = Result<()>>> {
        let series_id = data.series.id;
        let paths = self.paths.clone();

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

        let series = self.db.series.clone();

        let mut remotes = Vec::new();

        for (id, series_id) in &self.db.remote_series {
            remotes.push((series_id.clone(), RemoteId::Series { id: *id }));
        }

        for (id, series_id) in &self.db.remote_episodes {
            remotes.push((series_id.clone(), RemoteId::Episode { id: *id }));
        }

        // Remove any pending episodes for the given series.
        self.db.pending.retain(|p| p.series != series_id);
        let now = Utc::now();
        self.populate_pending(&now, series_id, None);
        let pending = self.db.pending.clone();

        Some(async move {
            let episodes = paths.episodes.join(format!("{}.json", series_id));
            let seasons = paths.seasons.join(format!("{}.json", series_id));
            let a = save_array("series", &paths.series, series.values());
            let b = save_array("episodes", &episodes, data.episodes);
            let c = save_array("seasons", &seasons, data.seasons);
            let d = save_array("remotes", &paths.remotes, remotes);
            let e = save_array("pending", &paths.pending, pending);
            tokio::try_join!(a, b, c, d, e)?;
            Ok(())
        })
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

    if let Some(watched) = load_array(&paths.watched)? {
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

    Ok(Some(load_array_from_reader(f)?))
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

        let f = std::fs::File::open(path)?;
        output.push((id, load_array_from_reader(f)?));
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
