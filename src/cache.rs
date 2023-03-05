use std::fmt;
use std::io;
use std::path::Path;

use anyhow::{bail, Result};
use iced_native::image::Handle;
use image_rs::GenericImageView;
use relative_path::RelativePath;

use crate::api::themoviedb;
use crate::api::thetvdb;
use crate::model::{ImageExt, ImageHash};

/// Whether or not to provide a scaled version of the image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum ImageHint {
    /// Specifies that the image should fit centered within the specified bounds.
    Fit(u32, u32),
    /// Fill the specified dimensions.
    Fill(u32, u32),
}

impl fmt::Display for ImageHint {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImageHint::Fit(w, h) => write!(f, "fit-{w}x{h}"),
            ImageHint::Fill(w, h) => write!(f, "fill-{w}x{h}"),
        }
    }
}

pub(crate) trait CacheClient<T: ?Sized> {
    async fn download_image(&self, id: &T) -> Result<Vec<u8>>;
}

impl CacheClient<RelativePath> for themoviedb::Client {
    #[inline]
    async fn download_image(&self, path: &RelativePath) -> Result<Vec<u8>> {
        themoviedb::Client::download_image_path(self, path).await
    }
}

impl CacheClient<RelativePath> for thetvdb::Client {
    #[inline]
    async fn download_image(&self, path: &RelativePath) -> Result<Vec<u8>> {
        thetvdb::Client::download_image_path(self, path).await
    }
}

pub(crate) trait CacheId {
    /// Get image extension.
    fn ext(&self) -> ImageExt;
}

impl CacheId for RelativePath {
    #[inline]
    fn ext(&self) -> ImageExt {
        match self.extension() {
            Some("jpg") => ImageExt::Jpg,
            _ => ImageExt::Unsupported,
        }
    }
}

/// Helper to load a cached image, or download it using the provided client if
/// needed.
pub(crate) async fn image<C, I>(
    path: &Path,
    client: &C,
    id: &I,
    hash: ImageHash,
    hint: Option<ImageHint>,
) -> Result<Handle>
where
    C: ?Sized + CacheClient<I>,
    I: ?Sized + fmt::Display + CacheId,
{
    use image_rs::imageops::FilterType;
    use std::io::Cursor;
    use tokio::fs;

    let format = match id.ext() {
        ImageExt::Jpg => image_rs::ImageFormat::Jpeg,
        ext => bail!("unsupported image format: {ext:?}"),
    };

    let (path, hint) = match hint {
        Some(hint) => {
            let resized_path = path.join(format!(
                "{:032x}-{hint}.{ext}",
                hash.as_u128(),
                ext = id.ext()
            ));
            (resized_path, Some(hint))
        }
        None => {
            let original_path = path.join(format!("{:032x}.{ext}", hash.as_u128(), ext = id.ext()));
            (original_path, None)
        }
    };

    match fs::read(&path).await {
        Ok(data) => {
            tracing::trace!("reading from cache: {}", path.display());
            let image = image_rs::load_from_memory_with_format(&data, format)?;
            let (width, height) = image.dimensions();
            let pixels = image.to_rgba8();
            return Ok(Handle::from_pixels(width, height, pixels.to_vec()));
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }

    tracing::debug!("downloading: {id}: {}", path.display());
    let data = client.download_image(&id).await?;
    let image = image_rs::load_from_memory_with_format(&data, format)?;

    let image = match hint {
        Some(hint) => {
            tokio::task::spawn_blocking(move || match hint {
                ImageHint::Fit(w, h) => image.resize_exact(w, h, FilterType::Lanczos3),
                ImageHint::Fill(w, h) => image.resize_to_fill(w, h, FilterType::Lanczos3),
            })
            .await?
        }
        None => image,
    };

    tracing::trace!("writing: {}", path.display());

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
