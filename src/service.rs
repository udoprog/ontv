use std::collections::{BTreeMap, HashMap};
use std::future::Future;
use std::path::{Path, PathBuf};

use anyhow::{Context, Error, Result};
use iced_native::image::Handle;
use serde::de::DeserializeOwned;
use serde::Serialize;
use uuid::Uuid;

use crate::message::Message;
use crate::model::{Image, RemoteSeriesId, Series, SeriesEpisode, SeriesId, SeriesSeason};
use crate::page;
use crate::thetvdb::Client;

static MISSING_BANNER: &[u8] = include_bytes!("../assets/missing_banner.png");

#[derive(Debug, Clone)]
pub(crate) struct SeriesData {
    id: SeriesId,
    series: Series,
    episodes: Vec<SeriesEpisode>,
    seasons: Vec<SeriesSeason>,
}

#[derive(Clone, Default)]
struct Database {
    thetvdb_ids: HashMap<SeriesId, Uuid>,
    series: HashMap<Uuid, Series>,
    episodes: HashMap<Uuid, Vec<SeriesEpisode>>,
    seasons: HashMap<Uuid, Vec<SeriesSeason>>,
}

/// Background service taking care of all state handling.
pub struct Service {
    /// Path to configuration file.
    config_path: PathBuf,
    /// Images configuration directory.
    images_dir: PathBuf,
    /// Path where series are stored.
    series_path: PathBuf,
    /// Path where episodes are stored.
    episodes_path: PathBuf,
    /// Path where seasons are stored.
    seasons_path: PathBuf,
    missing_banner: Handle,
    missing_poster: Handle,
    missing_screencap: Handle,
    db: Database,
    images: HashMap<Image, Handle>,
    /// Shared client.
    pub(crate) client: Client,
}

impl Service {
    /// Construct and setup in-memory state of
    pub(crate) fn new() -> Result<(Self, page::settings::Settings)> {
        let dirs = directories_next::ProjectDirs::from("se.tedro", "setbac", "OnTV")
            .context("missing project dirs")?;

        let config_path = dirs.config_dir().join("config.json");
        let series_path = dirs.config_dir().join("series.json");
        let episodes_path = dirs.config_dir().join("episodes");
        let seasons_path = dirs.config_dir().join("seasons");
        let images_dir = dirs.cache_dir().join("images");

        let missing_banner = Handle::from_memory(MISSING_BANNER);
        let missing_screencap = Handle::from_memory(MISSING_BANNER);
        let missing_poster = Handle::from_memory(MISSING_BANNER);

        let series = load_database(&series_path, &episodes_path, &seasons_path)?;

        let settings = match load_config(&config_path)? {
            Some(settings) => settings,
            None => Default::default(),
        };

        let client = Client::new(&settings.thetvdb_legacy_apikey);

        let this = Self {
            config_path,
            images_dir,
            series_path,
            episodes_path,
            seasons_path,
            missing_banner,
            missing_screencap,
            missing_poster,
            db: series,
            images: HashMap::new(),
            client,
        };

        Ok((this, settings))
    }

    /// Get a single series.
    pub(crate) fn series(&self, id: Uuid) -> Option<&Series> {
        self.db.series.get(&id)
    }

    /// Get list of series.
    pub(crate) fn list_series<'a>(&'a self) -> impl Iterator<Item = &'a Series> {
        self.db.series.values()
    }

    /// Iterator over available episodes.
    pub(crate) fn episodes<'a>(&'a self, id: Uuid) -> impl Iterator<Item = &'a SeriesEpisode> {
        self.db.episodes.get(&id).into_iter().flatten()
    }

    /// Iterator over available seasons.
    pub(crate) fn seasons<'a>(&'a self, id: Uuid) -> impl Iterator<Item = &'a SeriesSeason> {
        self.db.seasons.get(&id).into_iter().flatten()
    }

    /// Insert loaded images.
    pub(crate) fn insert_loaded_images(&mut self, loaded: Vec<(Image, Handle)>) {
        for (id, handle) in loaded {
            self.images.insert(id, handle);
        }
    }

    /// Setup background service, loading state from filesystem.
    pub(crate) fn setup(&self) -> impl Future<Output = Result<Vec<(Image, Handle)>>> {
        let client = self.client.clone();
        let images_dir = self.images_dir.clone();

        let mut ids = Vec::new();

        for s in self.db.series.values() {
            ids.push(s.poster);
            ids.extend(s.banner);
            ids.extend(s.fanart);
        }

        for e in self.db.episodes.values().flatten() {
            ids.extend(e.filename);
        }

        cache_images(client, images_dir, ids)
    }

    /// Load configuration file.
    pub(crate) fn save_config(
        &mut self,
        settings: page::settings::Settings,
    ) -> impl Future<Output = Message> {
        self.client.set_api_key(&settings.thetvdb_legacy_apikey);
        let config_path = self.config_path.clone();

        async move {
            match save_config(&config_path, &settings).await {
                Ok(()) => Message::SavedConfig,
                Err(e) => Message::error(e),
            }
        }
    }

    /// Get an image, will return the default handle if the given image doesn't exist.
    pub(crate) fn get_image(&self, id: &Image) -> Option<Handle> {
        self.images.get(&id).cloned()
    }

    /// Get a placeholder image for a missing banner.
    pub(crate) fn missing_banner(&self) -> Handle {
        self.missing_banner.clone()
    }

    /// Get a placeholder image for a missing poster.
    pub(crate) fn missing_poster(&self) -> Handle {
        self.missing_poster.clone()
    }

    /// Get a placeholder image for a missing screencap.
    pub(crate) fn missing_screencap(&self) -> Handle {
        self.missing_screencap.clone()
    }

    /// Check if series is tracked.
    pub(crate) fn is_thetvdb_tracked(&self, id: SeriesId) -> bool {
        self.db.thetvdb_ids.contains_key(&id)
    }

    /// Enable tracking of the series with the given id.
    pub(crate) fn track_thetvdb(
        &self,
        id: SeriesId,
    ) -> impl Future<Output = Result<(SeriesData, Vec<(Image, Handle)>)>> {
        fn seasons(episodes: &[SeriesEpisode]) -> Vec<SeriesSeason> {
            let mut map = BTreeMap::new();

            for e in episodes {
                map.entry(e.season)
                    .or_insert_with(|| SeriesSeason { number: e.season });
            }

            map.into_iter().map(|(_, value)| value).collect()
        }

        let client = self.client.clone();
        let images_dir = self.images_dir.clone();

        async move {
            let series = client.series(id);
            let episodes = client.series_episodes(id);

            let (series, episodes) = tokio::try_join!(series, episodes)?;

            let seasons = seasons(&episodes);

            let mut ids = Vec::new();

            ids.push(series.poster);
            ids.extend(series.banner);
            ids.extend(series.fanart);

            for e in &episodes {
                ids.extend(e.filename);
            }

            let output = cache_images(client, images_dir, ids).await?;

            let data = SeriesData {
                id,
                series,
                episodes,
                seasons,
            };

            Ok::<_, Error>((data, output))
        }
    }

    /// Insert a new tracked song.
    pub(crate) fn track(&mut self, data: SeriesData) -> Option<impl Future<Output = Result<()>>> {
        if self.db.thetvdb_ids.contains_key(&data.id) {
            return None;
        }

        let episodes_path = self.episodes_path.join(format!("{}.json", data.series.id));
        let seasons_path = self.seasons_path.join(format!("{}.json", data.series.id));

        self.db.thetvdb_ids.insert(data.id, data.series.id);
        self.db
            .episodes
            .insert(data.series.id, data.episodes.clone());
        self.db.seasons.insert(data.series.id, data.seasons.clone());
        self.db.series.insert(data.series.id, data.series);

        let series_path = self.series_path.clone();
        let series = self.db.series.clone();

        Some(async move {
            save_series(series_path, series).await?;
            save_array(episodes_path, data.episodes).await?;
            save_array(seasons_path, data.seasons).await?;
            Ok(())
        })
    }

    /// Disable tracking of the series with the given id.
    pub(crate) fn untrack(&mut self, id: SeriesId) -> Option<impl Future<Output = Result<()>>> {
        if !self.db.thetvdb_ids.contains_key(&id) {
            return None;
        }

        if let Some(id) = self.db.thetvdb_ids.remove(&id) {
            let _ = self.db.series.remove(&id);
            let _ = self.db.episodes.remove(&id);
        }

        let series_path = self.series_path.clone();
        let series = self.db.series.clone();
        Some(save_series(series_path, series))
    }

    /// Ensure that a collection of the given image ids are loaded.
    pub(crate) fn load_image(
        &self,
        id: Image,
    ) -> impl Future<Output = Result<Vec<(Image, Handle)>>> {
        let client = self.client.clone();
        let images_dir = self.images_dir.clone();

        let id = if !self.images.contains_key(&id) {
            Some(id)
        } else {
            None
        };

        cache_images(client, images_dir, id)
    }
}

/// Load configuration file.
pub(crate) fn load_config(path: &Path) -> Result<Option<page::settings::Settings>> {
    use std::fs;
    use std::io;

    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };

    Ok(serde_json::from_slice(&bytes)?)
}

/// Save configuration file.
pub(crate) async fn save_config(path: &Path, state: &page::settings::Settings) -> Result<()> {
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
async fn cache_images<I>(
    client: Client,
    images_dir: PathBuf,
    ids: I,
) -> Result<Vec<(Image, Handle)>>
where
    I: IntoIterator<Item = Image>,
{
    use tokio::fs;

    let mut output = Vec::new();

    for id in ids {
        let hash = id.hash();
        let cache_path = images_dir.join(format!("{:032x}.{}", hash, id.format()));

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

        log::debug!("downloaded: {id} ({} bytes)", data.len());
        let handle = Handle::from_memory(data);
        output.push((id, handle));
    }

    Ok(output)
}

/// Try to load initial state.
fn load_database(series: &Path, episodes: &Path, seasons: &Path) -> Result<Database> {
    let mut db = Database::default();

    if let Some(series) = load_series(series)? {
        for s in series {
            for remote_id in &s.remote_ids {
                match remote_id {
                    RemoteSeriesId::TheTvDb { id } => {
                        db.thetvdb_ids.insert(*id, s.id);
                    }
                }
            }

            db.series.insert(s.id, s);
        }
    }

    if let Some(episodes) = load_array(episodes)? {
        for (id, episodes) in episodes {
            db.episodes.insert(id, episodes);
        }
    }

    if let Some(seasons) = load_array(seasons)? {
        for (id, seasons) in seasons {
            db.seasons.insert(id, seasons);
        }
    }

    Ok(db)
}

/// Load series from the given path.
fn load_series(path: &Path) -> Result<Option<Vec<Series>>> {
    use std::fs;
    use std::io::{self, BufRead, BufReader};

    let f = match fs::File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };

    let mut output = Vec::new();

    for line in BufReader::new(f).lines() {
        let line = line?;
        let line = line.trim();

        if line.starts_with('#') || line.is_empty() {
            continue;
        }

        output.push(serde_json::from_str(&line)?);
    }

    Ok(Some(output))
}

/// Save series to the given path.
async fn save_series(path: PathBuf, data: HashMap<Uuid, Series>) -> Result<()> {
    use tokio::fs;
    use tokio::io::AsyncWriteExt;

    log::debug!("saving series to {}", path.display());

    if let Some(d) = path.parent() {
        if !matches!(fs::metadata(d).await, Ok(m) if m.is_dir()) {
            fs::create_dir_all(d).await?;
        }
    }

    let mut f = fs::File::create(path).await?;
    let mut line = Vec::new();

    for s in data.values() {
        line.clear();
        serde_json::to_writer(&mut line, s)?;
        line.push(b'\n');
        f.write_all(&line).await?;
    }

    Ok(())
}

/// Save series to the given path.
async fn save_array<T>(path: PathBuf, data: Vec<T>) -> Result<()>
where
    T: Serialize,
{
    use tokio::fs;
    use tokio::io::AsyncWriteExt;

    log::debug!("saving to {}", path.display());

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
fn load_array<T>(path: &Path) -> Result<Option<Vec<(Uuid, Vec<T>)>>>
where
    T: DeserializeOwned,
{
    use std::fs;
    use std::io::{self, BufRead, BufReader};

    let d = match fs::read_dir(path) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
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

        let f = fs::File::open(&path)?;
        let mut episodes = Vec::new();

        for line in BufReader::new(f).lines() {
            let line = line?;
            let line = line.trim();

            if line.starts_with('#') || line.is_empty() {
                continue;
            }

            episodes.push(serde_json::from_str(&line)?);
        }

        output.push((id, episodes));
    }

    Ok(Some(output))
}
