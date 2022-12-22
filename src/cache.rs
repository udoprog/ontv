use std::fmt;
use std::io;
use std::path::Path;

use anyhow::Result;
use iced_native::image::Handle;

use crate::api::themoviedb;
use crate::api::thetvdb;
use crate::model::{Image, ImageExt, TmdbImage, TvdbImage};

const TVDB: u64 = 0x907b86069129a824u64;
const TMDB: u64 = 0xd614d57a2eadc500u64;

pub(crate) trait CacheClient<T> {
    async fn download_image(&self, id: &T) -> Result<Vec<u8>>;
}

impl CacheClient<TmdbImage> for themoviedb::Client {
    #[inline]
    async fn download_image(&self, id: &TmdbImage) -> Result<Vec<u8>> {
        themoviedb::Client::download_image(self, id).await
    }
}

impl CacheClient<TvdbImage> for thetvdb::Client {
    #[inline]
    async fn download_image(&self, id: &TvdbImage) -> Result<Vec<u8>> {
        thetvdb::Client::downloage_image(self, id).await
    }
}

pub(crate) trait CacheId {
    /// Return 128-bit hash.
    fn hash128(&self) -> u128;

    /// Get image extension.
    fn ext(&self) -> ImageExt;
}

impl CacheId for TmdbImage {
    #[inline]
    fn hash128(&self) -> u128 {
        hash128(&(TMDB, self.kind))
    }

    #[inline]
    fn ext(&self) -> ImageExt {
        self.ext
    }
}

impl CacheId for TvdbImage {
    #[inline]
    fn hash128(&self) -> u128 {
        hash128(&(TVDB, self.kind))
    }

    #[inline]
    fn ext(&self) -> ImageExt {
        self.ext
    }
}

/// Helper to load a cached image, or download it using the provided client if
/// needed.
pub(crate) async fn image<I, C>(path: &Path, client: &C, id: I) -> Result<(Image, Handle)>
where
    C: CacheClient<I>,
    I: fmt::Display + CacheId,
    Image: From<I>,
{
    use tokio::fs;

    let hash = id.hash128();
    let cache_path = path.join(format!("{:032x}.{ext}", hash, ext = id.ext()));

    let data = match fs::read(&cache_path).await {
        Ok(data) => data,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            log::debug!("downloading: {id}: {}", cache_path.display());
            let data = client.download_image(&id).await?;

            if let Some(parent) = cache_path.parent() {
                if !matches!(fs::metadata(parent).await, Ok(m) if m.is_dir()) {
                    log::debug!("creating image cache directory: {}", parent.display());
                    fs::create_dir_all(parent).await?;
                }
            }

            fs::write(&cache_path, &data).await?;
            data
        }
        Err(e) => return Err(e.into()),
    };

    log::debug!("loaded: {id} ({} bytes)", data.len());
    let handle = Handle::from_memory(data);
    Ok((Image::from(id), handle))
}

/// Generate a 16-byte hash.
pub(crate) fn hash128<T>(value: &T) -> u128
where
    T: std::hash::Hash,
{
    use twox_hash::xxh3::HasherExt;
    let mut hasher = twox_hash::Xxh3Hash128::default();
    std::hash::Hash::hash(value, &mut hasher);
    hasher.finish_ext()
}
