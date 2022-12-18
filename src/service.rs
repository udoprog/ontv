use std::collections::HashMap;
use std::future::Future;
use std::path::{Path, PathBuf};

use anyhow::{Context, Error, Result};
use iced_native::image::Handle;
use uuid::Uuid;

use crate::message::Message;
use crate::model::{Image, RemoteSeriesId, Series, TheTvDbSeriesId};
use crate::page;
use crate::thetvdb::Client;

static MISSING_BANNER: &[u8] = include_bytes!("../assets/missing_banner.png");

#[derive(Clone, Default)]
struct SeriesState {
    thetvdb_ids: HashMap<TheTvDbSeriesId, Uuid>,
    data: HashMap<Uuid, Series>,
}

struct State {
    missing_banner: Handle,
    series: SeriesState,
    images: HashMap<Image, Handle>,
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
    // In-memory state of the service.
    state: State,
    /// Shared client.
    pub(crate) client: Client,
}

impl Service {
    /// Construct and setup in-memory state of
    pub(crate) fn new() -> Result<(Self, page::settings::State)> {
        let dirs = directories_next::ProjectDirs::from("se.tedro", "setbac", "OnTV")
            .context("missing project dirs")?;

        let config_path = dirs.config_dir().join("config.json");
        let series_path = dirs.config_dir().join("series.json");
        let episodes_path = dirs.config_dir().join("episodes");
        let images_dir = dirs.cache_dir().join("images");

        let missing_banner = Handle::from_memory(MISSING_BANNER);

        let series = load_initial_state(&series_path, &episodes_path)?;

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
            state: State {
                missing_banner,
                series,
                images: HashMap::new(),
            },
            client,
        };

        Ok((this, settings))
    }

    /// Get list of series.
    pub(crate) fn series<'a>(&'a self) -> impl IntoIterator<Item = &'a Series> {
        self.state.series.data.values()
    }

    /// Insert loaded images.
    pub(crate) fn insert_loaded_images(&mut self, loaded: Vec<(Image, Handle)>) {
        for (id, handle) in loaded {
            self.state.images.insert(id, handle);
        }
    }

    /// Setup background service, loading state from filesystem.
    pub(crate) fn setup(&self) -> impl Future<Output = Result<Vec<(Image, Handle)>>> {
        let client = self.client.clone();
        let images_dir = self.images_dir.clone();

        let mut ids = Vec::new();

        for s in self.state.series.data.values() {
            ids.push(s.poster);
            ids.extend(s.banner);
            ids.extend(s.fanart);
        }

        cache_images(client, images_dir, ids)
    }

    /// Load configuration file.
    pub(crate) fn save_config(
        &mut self,
        settings: page::settings::State,
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
    pub(crate) fn get_image(&self, id: &Image) -> Handle {
        let Some(image) = self.state.images.get(&id) else {
            return self.state.missing_banner.clone();
        };

        image.clone()
    }

    /// Check if series is tracked.
    pub(crate) fn is_thetvdb_tracked(&self, id: TheTvDbSeriesId) -> bool {
        self.state.series.thetvdb_ids.contains_key(&id)
    }

    /// Enable tracking of the series with the given id.
    pub(crate) fn track_thetvdb(
        &self,
        id: TheTvDbSeriesId,
    ) -> impl Future<Output = Result<(TheTvDbSeriesId, Series, Vec<(Image, Handle)>)>> {
        let client = self.client.clone();
        let images_dir = self.images_dir.clone();

        async move {
            let series = client.series(id).await?;

            let output = cache_images(
                client,
                images_dir,
                [series.poster]
                    .into_iter()
                    .chain(series.banner)
                    .chain(series.fanart),
            )
            .await?;

            Ok::<_, Error>((id, series, output))
        }
    }

    /// Insert a new tracked song.
    pub(crate) fn track(
        &mut self,
        id: TheTvDbSeriesId,
        series: Series,
    ) -> Option<impl Future<Output = Result<()>>> {
        if self.state.series.thetvdb_ids.contains_key(&id) {
            return None;
        }

        self.state.series.thetvdb_ids.insert(id, series.id);
        self.state.series.data.insert(series.id, series);

        let series_path = self.series_path.clone();
        let data = self.state.series.data.clone();
        Some(save_series(series_path, data))
    }

    /// Disable tracking of the series with the given id.
    pub(crate) fn untrack(
        &mut self,
        id: TheTvDbSeriesId,
    ) -> Option<impl Future<Output = Result<()>>> {
        if !self.state.series.thetvdb_ids.contains_key(&id) {
            return None;
        }

        if let Some(id) = self.state.series.thetvdb_ids.remove(&id) {
            let _ = self.state.series.data.remove(&id);
        }

        let series_path = self.series_path.clone();
        let data = self.state.series.data.clone();
        Some(save_series(series_path, data))
    }

    /// Ensure that a collection of the given image ids are loaded.
    pub(crate) fn load_image(
        &self,
        id: Image,
    ) -> impl Future<Output = Result<Vec<(Image, Handle)>>> {
        let client = self.client.clone();
        let images_dir = self.images_dir.clone();

        let id = if !self.state.images.contains_key(&id) {
            Some(id)
        } else {
            None
        };

        cache_images(client, images_dir, id)
    }
}

/// Load configuration file.
pub(crate) fn load_config(path: &Path) -> Result<Option<page::settings::State>> {
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
pub(crate) async fn save_config(path: &Path, state: &page::settings::State) -> Result<()> {
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
fn load_initial_state(series: &Path, episodes: &Path) -> Result<SeriesState> {
    let mut state = SeriesState::default();

    if let Some(series) = load_series(series)? {
        for s in series {
            for remote_id in &s.remote_ids {
                match remote_id {
                    RemoteSeriesId::TheTvDb { id } => {
                        state.thetvdb_ids.insert(*id, s.id);
                    }
                }
            }

            state.data.insert(s.id, s);
        }
    }

    Ok(state)
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
