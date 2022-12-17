use std::collections::HashMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::Mutex;

use anyhow::{Context, Error, Result};
use iced_native::image::Handle;

use crate::message::Message;
use crate::model::Image;
use crate::page;
use crate::thetvdb;

static MISSING_BANNER: &[u8] = include_bytes!("../assets/missing_banner.png");

struct State {
    missing_banner: Handle,
    images: Mutex<HashMap<Image, Handle>>,
}

/// Background service taking care of all state handling.
pub struct Service {
    /// Path to configuration file.
    config_path: PathBuf,
    /// Images configuration directory.
    images_dir: PathBuf,
    // In-memory state of the service.
    state: Arc<State>,
    /// Shared client.
    pub(crate) client: thetvdb::Client,
}

impl Service {
    /// Construct and setup in-memory state of
    pub(crate) fn new() -> Result<Self> {
        let dirs = directories_next::ProjectDirs::from("se.tedro", "setbac", "OnTV")
            .context("missing project dirs")?;

        let config_path = dirs.config_dir().join("config.json");
        let images_dir = dirs.cache_dir().join("images");

        let missing_banner = Handle::from_memory(MISSING_BANNER);

        Ok(Self {
            config_path,
            images_dir,
            state: Arc::new(State {
                missing_banner,
                images: Mutex::new(HashMap::new()),
            }),
            client: thetvdb::Client::new(),
        })
    }

    /// Setup background service, loading state from filesystem.
    pub(crate) fn setup(&self) -> impl Future<Output = Message> + 'static {
        let config_path = self.config_path.clone();
        let client = self.client.clone();

        async move {
            let (settings, error) = match load_config(&config_path).await {
                Ok(Some(settings)) => (settings, None),
                Ok(None) => (Default::default(), None),
                Err(error) => {
                    log::error!("failed to load config: {}: {error}", config_path.display());
                    (Default::default(), Some(Arc::new(error)))
                }
            };

            client.set_api_key(&settings.thetvdb_legacy_apikey);
            Message::Setup((settings, error))
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

    /// Ensure that a collection of the given image ids are loaded.
    pub(crate) fn load_images(&self, ids: &[Image]) -> impl Future<Output = Message> {
        let state = self.state.clone();

        let mut op = async move { Ok::<_, Error>(()) };

        async move {
            match op.await {
                Ok(()) => Message::ImagesLoaded,
                Err(e) => Message::Error(e.to_string()),
            }
        }
    }
}

/// Load configuration file.
pub(crate) async fn load_config(path: &Path) -> Result<Option<page::settings::State>> {
    use std::io;
    use tokio::fs;

    let bytes = match fs::read(path).await {
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
