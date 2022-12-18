use std::collections::HashMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::Mutex;

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
    series: Mutex<SeriesState>,
    images: Mutex<HashMap<Image, Handle>>,
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
    state: Arc<State>,
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

        let client = Client::new();
        client.set_api_key(&settings.thetvdb_legacy_apikey);

        let this = Self {
            config_path,
            images_dir,
            series_path,
            episodes_path,
            state: Arc::new(State {
                missing_banner,
                series: Mutex::new(series),
                images: Mutex::new(HashMap::new()),
            }),
            client,
        };

        Ok((this, settings))
    }

    /// Get list of series.
    pub(crate) fn series(&self) -> Vec<Series> {
        let series = self
            .state
            .series
            .lock()
            .unwrap()
            .data
            .values()
            .cloned()
            .collect::<Vec<_>>();

        series
    }

    /// Setup background service, loading state from filesystem.
    pub(crate) fn setup(&self) -> impl Future<Output = Message> + 'static {
        let client = self.client.clone();
        let state = self.state.clone();
        let images_dir = self.images_dir.clone();

        let op = async move {
            let mut ids = Vec::new();

            for s in state.series.lock().unwrap().data.values() {
                ids.push(s.poster);
                ids.extend(s.banner);
                ids.extend(s.fanart);
            }

            cache_images(&state, &client, &images_dir, ids).await?;
            Ok::<_, Error>(Message::ImageLoaded)
        };

        async move {
            match op.await {
                Ok(m) => m,
                Err(e) => Message::error(e),
            }
        }
    }

    /// Load configuration file.
    pub(crate) fn save_config(
        &self,
        settings: page::settings::State,
    ) -> impl Future<Output = Message> + 'static {
        let config_path = self.config_path.clone();
        let client = self.client.clone();

        async move {
            let ok = if let Err(error) = save_config(&config_path, &settings).await {
                log::error!("failed to save config: {}: {error}", config_path.display());
                false
            } else {
                true
            };

            client.set_api_key(&settings.thetvdb_legacy_apikey);
            Message::SavedConfig(ok)
        }
    }

    /// Get an image, will return the default handle if the given image doesn't exist.
    pub(crate) fn get_image(&self, id: &Image) -> Handle {
        let images = self.state.images.lock().unwrap();

        let Some(image) = images.get(&id) else {
            return self.state.missing_banner.clone();
        };

        image.clone()
    }

    /// Check if series is tracked.
    pub(crate) fn is_thetvdb_tracked(&self, id: TheTvDbSeriesId) -> bool {
        self.state
            .series
            .lock()
            .unwrap()
            .thetvdb_ids
            .contains_key(&id)
    }

    /// Enable tracking of the series with the given id.
    pub(crate) fn track_thetvdb(&self, id: TheTvDbSeriesId) -> impl Future<Output = Message> {
        let state = self.state.clone();
        let client = self.client.clone();
        let images_dir = self.images_dir.clone();
        let series_path = self.series_path.clone();

        let op = async move {
            if state.series.lock().unwrap().thetvdb_ids.contains_key(&id) {
                return Ok::<_, Error>(());
            }

            let series = client.series(id).await?;

            cache_images(
                &state,
                &client,
                &images_dir,
                [series.poster]
                    .into_iter()
                    .chain(series.banner)
                    .chain(series.fanart),
            )
            .await?;

            let data = {
                let mut s = state.series.lock().unwrap();
                s.thetvdb_ids.insert(id, series.id);
                s.data.insert(series.id, series);
                s.data.clone()
            };

            save_series(&series_path, &data).await?;
            Ok::<_, Error>(())
        };

        async move {
            match op.await {
                Ok(()) => Message::SeriesTracked,
                Err(e) => Message::error(e),
            }
        }
    }

    /// Disable tracking of the series with the given id.
    pub(crate) fn untrack(&self, id: TheTvDbSeriesId) -> impl Future<Output = Message> {
        let state = self.state.clone();

        let op = async move {
            if !state.series.lock().unwrap().thetvdb_ids.contains_key(&id) {
                return Ok::<_, Error>(());
            }

            let mut series = state.series.lock().unwrap();

            if let Some(id) = series.thetvdb_ids.remove(&id) {
                let _ = series.data.remove(&id);
            }

            Ok::<_, Error>(())
        };

        async move {
            match op.await {
                Ok(()) => Message::SeriesTracked,
                Err(e) => Message::error(e),
            }
        }
    }

    /// Ensure that a collection of the given image ids are loaded.
    pub(crate) fn load_image(&self, id: Image) -> impl Future<Output = Message> {
        let state = self.state.clone();
        let client = self.client.clone();
        let images_dir = self.images_dir.clone();

        let op = async move {
            cache_images(&state, &client, &images_dir, [id]).await?;
            Ok::<_, Error>(())
        };

        async move {
            match op.await {
                Ok(()) => Message::ImageLoaded,
                Err(e) => Message::error(e),
            }
        }
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
async fn cache_images<I>(state: &State, client: &Client, images_dir: &Path, ids: I) -> Result<()>
where
    I: IntoIterator<Item = Image>,
{
    use tokio::fs;

    for id in ids {
        if state.images.lock().unwrap().contains_key(&id) {
            continue;
        }

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
        state.images.lock().unwrap().insert(id, handle);
    }

    Ok(())
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
async fn save_series(path: &Path, data: &HashMap<Uuid, Series>) -> Result<()> {
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