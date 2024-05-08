use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use api::{ImageHint, ImageV2};
use image::ImageFormat;
use tokio::sync::{oneshot, Mutex};

use crate::api::{themoviedb, thetvdb};
use crate::assets::ImageKey;
use crate::cache;
use crate::service::paths::Paths;

#[derive(Clone)]
pub(super) struct ImageCache {
    paths: Arc<Paths>,
    tvdb: thetvdb::Client,
    tmdb: themoviedb::Client,
    lock: Arc<Mutex<HashMap<ImageKey, Vec<oneshot::Sender<()>>>>>,
}

impl ImageCache {
    pub(super) fn new(paths: Arc<Paths>, tvdb: thetvdb::Client, tmdb: themoviedb::Client) -> Self {
        Self {
            paths,
            tvdb,
            tmdb,
            lock: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    async fn wait(&self, key: ImageKey) {
        let mut receiver = None::<oneshot::Receiver<()>>;

        loop {
            if let Some(receiver) = receiver.take() {
                receiver.await.ok();
            }

            let mut map = self.lock.lock().await;

            match map.entry(key) {
                Entry::Occupied(mut e) => {
                    let (tx, rx) = oneshot::channel();
                    e.into_mut().push(tx);
                    receiver = Some(rx);
                }
                Entry::Vacant(e) => {
                    e.insert(Vec::new());
                    return;
                }
            }
        }
    }

    async fn release(&self, key: ImageKey) {
        self.lock.lock().await.remove(&key);
    }

    /// Ensure that a collection of the given image ids are loaded.
    pub(super) async fn load_image(
        &self,
        image: ImageV2,
        hint: ImageHint,
    ) -> Result<(Vec<u8>, ImageFormat)> {
        let hash = image.hash();
        let key = ImageKey { id: hash, hint };

        self.wait(key).await;

        let handle = match &image {
            ImageV2::Tvdb { uri } => {
                cache::image(&self.paths.images, &self.tvdb, uri.as_ref(), hash, hint).await
            }
            ImageV2::Tmdb { uri } => {
                cache::image(&self.paths.images, &self.tmdb, uri.as_ref(), hash, hint).await
            }
        };

        self.release(key).await;
        let handle = handle.with_context(|| anyhow!("Downloading: {image:?}"))?;
        Ok(handle)
    }
}
