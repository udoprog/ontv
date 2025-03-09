use std::fmt;
use std::future::Future;
use std::io;
use std::path::Path;
use std::pin::Pin;

use anyhow::{bail, Result};
use iced::advanced::image::Handle;
use image_rs::imageops::FilterType;
use image_rs::{DynamicImage, GenericImageView};
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
    fn download_image(
        &self,
        id: &T,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send + 'static>>;
}

impl CacheClient<RelativePath> for themoviedb::Client {
    #[inline]
    fn download_image(
        &self,
        path: &RelativePath,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send + 'static>> {
        let client = self.clone();
        let path: Box<RelativePath> = path.into();
        Box::pin(async move { themoviedb::Client::download_image_path(&client, &path).await })
    }
}

impl CacheClient<RelativePath> for thetvdb::Client {
    #[inline]
    fn download_image(
        &self,
        path: &RelativePath,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send + 'static>> {
        let client = self.clone();
        let path: Box<RelativePath> = path.into();
        Box::pin(async move { thetvdb::Client::download_image_path(&client, &path).await })
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
    use std::io::Cursor;
    use tokio::fs;

    let format = match id.ext() {
        ImageExt::Jpg => image_rs::ImageFormat::Jpeg,
        ext => bail!("Unsupported image format: {ext:?}"),
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
            tracing::trace!(path = path.display().to_string(), "Reading from cache");
            let image = image_rs::load_from_memory_with_format(&data, format)?;
            let (width, height) = image.dimensions();
            let pixels = image.to_rgba8();
            return Ok(Handle::from_rgba(width, height, pixels.to_vec()));
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }

    tracing::debug!(
        id = id.to_string(),
        path = path.display().to_string(),
        "Downloading"
    );

    let data = client.download_image(id).await?;
    let image = image_rs::load_from_memory_with_format(&data, format)?;

    let image = match hint {
        Some(hint) => {
            tokio::task::spawn_blocking(move || match hint {
                ImageHint::Fit(w, h) => image.resize_exact(w, h, FilterType::Lanczos3),
                ImageHint::Fill(w, h) => resize_to_fill_top(image, w, h, FilterType::Lanczos3),
            })
            .await?
        }
        None => image,
    };

    tracing::trace!("Writing: {}", path.display());

    let mut buf = Cursor::new(Vec::with_capacity(1024));
    image.write_to(&mut buf, format)?;
    fs::write(&path, buf.into_inner()).await?;

    let (width, height) = image.dimensions();
    let pixels = image.to_rgba8();
    Ok(Handle::from_rgba(width, height, pixels.to_vec()))
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

/// Resize to fill but preserves the top of the image rather than centers it.
pub fn resize_to_fill_top(
    image: DynamicImage,
    nwidth: u32,
    nheight: u32,
    filter: FilterType,
) -> DynamicImage {
    let (width2, height2) = resize_dimensions(image.width(), image.height(), nwidth, nheight, true);

    let mut intermediate = image.resize_exact(width2, height2, filter);
    let (iwidth, iheight) = intermediate.dimensions();
    let ratio = u64::from(iwidth) * u64::from(nheight);
    let nratio = u64::from(nwidth) * u64::from(iheight);

    if nratio > ratio {
        intermediate.crop(0, 0, nwidth, nheight)
    } else {
        intermediate.crop((iwidth - nwidth) / 2, 0, nwidth, nheight)
    }
}

pub(crate) fn resize_dimensions(
    width: u32,
    height: u32,
    nwidth: u32,
    nheight: u32,
    fill: bool,
) -> (u32, u32) {
    use std::cmp::max;

    let wratio = nwidth as f64 / width as f64;
    let hratio = nheight as f64 / height as f64;

    let ratio = if fill {
        f64::max(wratio, hratio)
    } else {
        f64::min(wratio, hratio)
    };

    let nw = max((width as f64 * ratio).round() as u64, 1);
    let nh = max((height as f64 * ratio).round() as u64, 1);

    if nw > u64::from(u32::MAX) {
        let ratio = u32::MAX as f64 / width as f64;
        (u32::MAX, max((height as f64 * ratio).round() as u32, 1))
    } else if nh > u64::from(u32::MAX) {
        let ratio = u32::MAX as f64 / height as f64;
        (max((width as f64 * ratio).round() as u32, 1), u32::MAX)
    } else {
        (nw as u32, nh as u32)
    }
}
