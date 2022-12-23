use std::fmt;
use std::io;
use std::path::Path;

use anyhow::Result;
use iced_native::image::Handle;
use image_rs::GenericImageView;

use crate::api::themoviedb;
use crate::api::thetvdb;
use crate::model::{ImageExt, TmdbImage, TvdbImage};

const TVDB: u64 = 0x907b86069129a824u64;
const TMDB: u64 = 0xd614d57a2eadc500u64;

/// Whether or not to provide a scaled version of the image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum ImageHint {
    /// Ensure that the image is scaled so that it has a max width in the
    /// specified number of pixels.
    Width(u32),
    /// Ensure that the image is scaled so that it has a max height in the
    /// specified number of pixels.
    Height(u32),
    /// Specifies a maximum width and height.
    Max(u32),
}

impl fmt::Display for ImageHint {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImageHint::Width(px) => write!(f, "w{px}"),
            ImageHint::Height(px) => write!(f, "h{px}"),
            ImageHint::Max(px) => write!(f, "x{px}"),
        }
    }
}

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
pub(crate) async fn image<I, C>(
    path: &Path,
    client: &C,
    id: I,
    hint: Option<ImageHint>,
) -> Result<Handle>
where
    C: CacheClient<I>,
    I: fmt::Display + CacheId,
{
    use image_rs::imageops::FilterType;
    use std::io::Cursor;
    use tokio::fs;

    let hash = id.hash128();

    let format = match id.ext() {
        ImageExt::Jpg => image_rs::ImageFormat::Jpeg,
    };

    let (path, hint) = match hint {
        Some(hint) => {
            let resized_path = path.join(format!("{:032x}-{hint}.{ext}", hash, ext = id.ext()));
            (resized_path, Some(hint))
        }
        None => {
            let original_path = path.join(format!("{:032x}.{ext}", hash, ext = id.ext()));
            (original_path, None)
        }
    };

    match fs::read(&path).await {
        Ok(data) => {
            log::trace!("reading from cache: {}", path.display());
            let image = image_rs::load_from_memory_with_format(&data, format)?;
            let (width, height) = image.dimensions();
            let pixels = image.to_rgba8();
            return Ok(Handle::from_pixels(width, height, pixels.to_vec()));
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }

    log::debug!("downloading: {id}: {}", path.display());
    let data = client.download_image(&id).await?;
    let image = image_rs::load_from_memory_with_format(&data, format)?;
    let (width, height) = image.dimensions();

    let image = match hint {
        Some(hint) => {
            tokio::task::spawn_blocking(move || match hint {
                ImageHint::Width(px) => image.resize(px, height, FilterType::Lanczos3),
                ImageHint::Height(px) => image.resize(width, px, FilterType::Lanczos3),
                ImageHint::Max(px) => image.resize(px, px, FilterType::Lanczos3),
            })
            .await?
        }
        None => image,
    };

    log::trace!("writing: {}", path.display());

    let mut buf = Cursor::new(Vec::with_capacity(1024));
    image.write_to(&mut buf, format)?;
    fs::write(&path, buf.into_inner()).await?;

    let (width, height) = image.dimensions();
    let pixels = image.to_rgba8();
    Ok(Handle::from_pixels(width, height, pixels.to_vec()))
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
