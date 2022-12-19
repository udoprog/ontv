use std::collections::{BTreeMap, HashMap, VecDeque};
use std::future::Future;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Error, Result};
use iced_native::image::Handle;
use serde::de::DeserializeOwned;
use serde::Serialize;
use uuid::Uuid;

use crate::message::Message;
use crate::model::{
    Episode, Image, RemoteEpisodeId, RemoteId, RemoteSeriesId, Season, Series, Watched,
};
use crate::page::settings::Settings;
use crate::thetvdb::Client;

static MISSING_BANNER: &[u8] = include_bytes!("../assets/missing_banner.png");

/// Data encapsulating a newly added series.
#[derive(Debug, Clone)]
pub(crate) struct NewSeries {
    series: Series,
    episodes: Vec<Episode>,
    seasons: Vec<Season>,
}

#[derive(Clone, Default)]
struct Database {
    remote_series: BTreeMap<RemoteSeriesId, Uuid>,
    remote_episodes: BTreeMap<RemoteEpisodeId, Uuid>,
    series: BTreeMap<Uuid, Series>,
    episodes: HashMap<Uuid, Vec<Episode>>,
    seasons: HashMap<Uuid, Vec<Season>>,
    watched: Vec<Watched>,
}

/// Background service taking care of all state handling.
pub struct Service {
    /// Path to configuration file.
    config_path: PathBuf,
    /// Path where remote mappings are stored.
    remotes_path: PathBuf,
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
    /// If there are new images to load.
    pub(crate) new_images: bool,
    /// Image queue to load.
    pub(crate) image_ids: VecDeque<Image>,
}

impl Service {
    /// Construct and setup in-memory state of
    pub(crate) fn new() -> Result<(Self, Settings)> {
        let dirs = directories_next::ProjectDirs::from("se.tedro", "setbac", "OnTV")
            .context("missing project dirs")?;

        let config_path = dirs.config_dir().join("config.json");
        let remotes_path = dirs.config_dir().join("remotes.json");
        let series_path = dirs.config_dir().join("series.json");
        let episodes_path = dirs.config_dir().join("episodes");
        let seasons_path = dirs.config_dir().join("seasons");
        let images_dir = dirs.cache_dir().join("images");

        let missing_banner = Handle::from_memory(MISSING_BANNER);
        let missing_screencap = Handle::from_memory(MISSING_BANNER);
        let missing_poster = Handle::from_memory(MISSING_BANNER);

        let db = load_database(&remotes_path, &series_path, &episodes_path, &seasons_path)?;

        let settings = match load_config(&config_path)? {
            Some(settings) => settings,
            None => Default::default(),
        };

        let client = Client::new(&settings.thetvdb_legacy_apikey);

        let this = Self {
            config_path,
            remotes_path,
            images_dir,
            series_path,
            episodes_path,
            seasons_path,
            missing_banner,
            missing_screencap,
            missing_poster,
            db,
            images: HashMap::new(),
            client,
            new_images: false,
            image_ids: VecDeque::new(),
        };

        Ok((this, settings))
    }

    /// Setup images to load task.
    pub(crate) fn mark_images<I>(&mut self, images: I)
    where
        I: IntoIterator<Item = Image>,
    {
        self.image_ids.clear();

        let mut all_loaded = true;

        for image in images {
            self.image_ids.push_back(image);

            if !self.images.contains_key(&image) {
                all_loaded = false;
            }
        }

        if all_loaded {
            self.image_ids.clear();
            return;
        }

        self.new_images = true;
        // NB: important to free the memory of images we are no longer using.
        self.images.clear();
    }

    /// Get a single series.
    pub(crate) fn series(&self, id: Uuid) -> Option<&Series> {
        self.db.series.get(&id)
    }

    /// Get list of series.
    pub(crate) fn all_series<'a>(&'a self) -> impl Iterator<Item = &'a Series> {
        self.db.series.values()
    }

    /// Iterator over available episodes.
    pub(crate) fn episodes<'a>(&'a self, id: Uuid) -> impl Iterator<Item = &'a Episode> {
        self.db.episodes.get(&id).into_iter().flatten()
    }

    /// Iterator over available seasons.
    pub(crate) fn seasons<'a>(&'a self, id: Uuid) -> impl Iterator<Item = &'a Season> {
        self.db.seasons.get(&id).into_iter().flatten()
    }

    /// Insert loaded images.
    pub(crate) fn insert_loaded_images(&mut self, loaded: Vec<(Image, Handle)>) {
        for (id, handle) in loaded {
            self.images.insert(id, handle);
        }
    }

    /// Load configuration file.
    pub(crate) fn save_config(&mut self, settings: Settings) -> impl Future<Output = Message> {
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
    pub(crate) fn get_series_by_remote(&self, id: RemoteSeriesId) -> Option<&Series> {
        let id = self.db.remote_series.get(&id)?;
        self.db.series.get(id)
    }

    /// Enable tracking of the series with the given id.
    pub(crate) fn track_by_remote(
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

        let series_path = self.series_path.clone();
        let series = self.db.series.clone();

        Some(save_array(
            "series",
            series_path,
            series.into_iter().map(|d| d.1),
        ))
    }

    /// Disable tracking of the series with the given id.
    pub(crate) fn untrack(&mut self, id: Uuid) -> Option<impl Future<Output = Result<()>>> {
        let s = self.db.series.get_mut(&id)?;
        s.tracked = false;

        let series_path = self.series_path.clone();
        let series = self.db.series.clone();

        Some(save_array(
            "series",
            series_path,
            series.into_iter().map(|d| d.1),
        ))
    }

    /// Insert a new tracked song.
    pub(crate) fn insert_new_series(
        &mut self,
        data: NewSeries,
    ) -> Option<impl Future<Output = Result<()>>> {
        let remotes_path = self.remotes_path.clone();
        let episodes_path = self.episodes_path.join(format!("{}.json", data.series.id));
        let seasons_path = self.seasons_path.join(format!("{}.json", data.series.id));

        for remote_id in &data.series.remote_ids {
            self.db.remote_series.insert(*remote_id, data.series.id);
        }

        for episode in &data.episodes {
            for remote_id in &episode.remote_ids {
                self.db.remote_episodes.insert(*remote_id, episode.id);
            }
        }

        self.db
            .episodes
            .insert(data.series.id, data.episodes.clone());

        self.db.seasons.insert(data.series.id, data.seasons.clone());
        self.db.series.insert(data.series.id, data.series);

        let series_path = self.series_path.clone();
        let series = self.db.series.clone();

        let mut remotes = Vec::new();

        for (id, series_id) in &self.db.remote_series {
            remotes.push((series_id.clone(), RemoteId::Series { id: *id }));
        }

        for (id, series_id) in &self.db.remote_episodes {
            remotes.push((series_id.clone(), RemoteId::Episode { id: *id }));
        }

        Some(async move {
            save_array("series", series_path, series.values()).await?;
            save_array("remotes", remotes_path, remotes).await?;
            save_array("episodes", episodes_path, data.episodes).await?;
            save_array("seasons", seasons_path, data.seasons).await?;
            Ok(())
        })
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

        log::debug!("loaded: {id} ({} bytes)", data.len());
        let handle = Handle::from_memory(data);
        output.push((id, handle));
    }

    Ok(output)
}

/// Try to load initial state.
fn load_database(
    remotes: &Path,
    series: &Path,
    episodes: &Path,
    seasons: &Path,
) -> Result<Database> {
    let mut db = Database::default();

    if let Some(remotes) = load_array::<(Uuid, RemoteId)>(remotes)? {
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

    if let Some(series) = load_series(series)? {
        for s in series {
            for &id in &s.remote_ids {
                db.remote_series.insert(id, s.id);
            }

            db.series.insert(s.id, s);
        }
    }

    if let Some(episodes) = load_directory(episodes)? {
        for (id, episodes) in episodes {
            db.episodes.insert(id, episodes);
        }
    }

    if let Some(seasons) = load_directory(seasons)? {
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

/// Save series to the given path.
async fn save_array<I>(what: &'static str, path: PathBuf, data: I) -> Result<()>
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
