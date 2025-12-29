use std::fmt;
use std::future::Future;
use std::hash::BuildHasherDefault;
use std::hash::Hasher;
use std::io;
use std::path::{Path, PathBuf};
use std::pin::Pin;

use anyhow::{anyhow, bail, Result};
use api::{ImageExt, ImageHash, ImageHint, ImageSizeHint};
use image::imageops::FilterType;
use image::GenericImageView;
use relative_path::RelativePath;
use tokio::task;

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
) -> Result<PathBuf>
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

    let (path, hint) = match hint {
        ImageHint::Original => {
            let original_path = path.join(format!("{hash}.{ext}", ext = id.ext()));
            (original_path, ImageHint::Original)
        }
        hint => {
            let resized_path = path.join(format!("{hash}-{hint}.{ext}", ext = id.ext()));
            (resized_path, hint)
        }
    };

    match fs::metadata(&path).await {
        Ok(m) if m.is_file() => {
            tracing::trace!(path = path.display().to_string(), "Reading from cache");
            return Ok(path);
        }
        Ok(..) => {
            return Err(anyhow!("Not a file: {}", path.display()));
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }

    tracing::info!(
        id = id.to_string(),
        path = path.display().to_string(),
        "downloading"
    );

    let data = client.download_image(id).await?;

    let data = match hint {
        ImageHint::Resize(hint) => {
            task::spawn_blocking(move || {
                let image = image::load_from_memory_with_format(&data, format)?;

                let image = match hint {
                    ImageSizeHint::Fit(w, h) => image.resize_exact(w, h, FilterType::Lanczos3),
                    ImageSizeHint::Fill(w, h) => {
                        resize_to_fill_top(image, w, h, FilterType::Lanczos3)
                    }
                };

                let mut buf = Cursor::new(Vec::with_capacity(1024));
                image.write_to(&mut buf, format)?;
                Ok::<_, anyhow::Error>(buf.into_inner())
            })
            .await??
        }
        ImageHint::Original => data,
    };

    tracing::trace!("Writing: {}", path.display());
    fs::write(&path, data).await?;
    Ok(path)
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
