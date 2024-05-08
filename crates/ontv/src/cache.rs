use std::fmt;
use std::future::Future;
use std::io;
use std::path::Path;
use std::pin::Pin;

use anyhow::{bail, Result};
use api::{ImageExt, ImageHash, ImageHint};
use image::imageops::FilterType;
use image::GenericImageView;
use image::ImageFormat;
use relative_path::RelativePath;

use crate::api::themoviedb;
use crate::api::thetvdb;

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
    hint: ImageHint,
) -> Result<(Vec<u8>, ImageFormat)>
where
    C: ?Sized + CacheClient<I>,
    I: ?Sized + fmt::Display + CacheId,
{
    use std::io::Cursor;
    use tokio::fs;

    let format = match id.ext() {
        ImageExt::Jpg => image::ImageFormat::Jpeg,
        ext => bail!("Unsupported image format: {ext:?}"),
    };

    let path = path.join(format!(
        "{:032x}{hint}.{ext}",
        hash.as_u128(),
        ext = id.ext()
    ));

    match fs::read(&path).await {
        Ok(data) => {
            tracing::trace!(path = path.display().to_string(), "Reading from cache");
            return Ok((data, format));
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
    let image = image::load_from_memory_with_format(&data, format)?;

    let image = match hint {
        ImageHint::Raw => image,
        hint => {
            tokio::task::spawn_blocking(move || match hint {
                ImageHint::Fit(w, h) => {
                    let (width, height) =
                        resize_dimensions(image.width(), image.height(), w, h, true);
                    image.resize_exact(width, height, FilterType::Lanczos3)
                }
                ImageHint::Fill(w, h) => resize_to_fill_top(image, w, h, FilterType::Lanczos3),
                ImageHint::Raw => image,
            })
            .await?
        }
    };

    tracing::trace!("Writing: {}", path.display());

    let mut buf = Cursor::new(Vec::with_capacity(1024));
    image.write_to(&mut buf, format)?;
    let buf = buf.into_inner();
    fs::write(&path, &buf).await?;
    Ok((buf, format))
}

/// Resize to fill but preserves the top of the image rather than centers it.
pub fn resize_to_fill_top(
    image: image::DynamicImage,
    nwidth: u32,
    nheight: u32,
    filter: FilterType,
) -> image::DynamicImage {
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

fn resize_dimensions(width: u32, height: u32, nwidth: u32, nheight: u32, fill: bool) -> (u32, u32) {
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
