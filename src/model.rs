mod hex16;
mod raw16;

#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use anyhow::{bail, ensure, Context, Result};
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub(crate) use self::hex16::Hex16;
pub(crate) use self::raw16::Raw16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "type")]
pub(crate) enum RemoteSeriesId {
    TheTvDb { id: TheTvDbSeriesId },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Source {
    TheTvDb,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(transparent)]
#[serde(transparent)]
pub(crate) struct TheTvDbSeriesId(u64);

impl From<u64> for TheTvDbSeriesId {
    #[inline]
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl fmt::Display for TheTvDbSeriesId {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// A single episode in a series.
#[derive(Debug, Clone)]
#[allow(unused)]
pub(crate) struct Episode {
    title: Arc<str>,
    season: u32,
    number: u32,
    series: Hex16,
}

/// A series.
#[derive(Debug, Clone)]
pub(crate) struct Series {
    /// Allocated UUID.
    pub(crate) id: Uuid,
    /// Title of the series.
    pub(crate) title: String,
    /// Poster image.
    pub(crate) poster: Image,
    /// Banner image.
    pub(crate) banner: Option<Image>,
    /// Fanart image.
    pub(crate) fanart: Option<Image>,
    /// Remote series ids.
    #[allow(unused)]
    pub(crate) remote_ids: Vec<RemoteSeriesId>,
    // Raw API response in case we need to reconstruct something later.
    #[allow(unused)]
    pub(crate) raw: HashMap<Source, Bytes>,
}

/// Image format in use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ImageFormat {
    Jpg,
}

impl ImageFormat {
    /// Parse a banner URL.
    fn parse(input: &str) -> Result<Self> {
        match input {
            "jpg" => Ok(ImageFormat::Jpg),
            _ => {
                bail!("{input}: unsupported image format")
            }
        }
    }
}

impl fmt::Display for ImageFormat {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImageFormat::Jpg => write!(f, "jpg"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub(crate) struct Image {
    pub(crate) kind: ImageKind,
    pub(crate) format: ImageFormat,
}

impl Image {
    /// Parse an image URL from thetvdb.
    pub(crate) fn parse(mut input: &str) -> Result<Self> {
        input = input.trim_start_matches('/');

        let mut it = input.split('/');

        ensure!(
            matches!(it.next(), Some("banners")),
            "{input}: missing `banners`"
        );

        Self::parse_banner_it(input, it)
    }

    /// Parse without expecting a `banners` prefix.
    #[inline]
    pub(crate) fn parse_banner(input: &str) -> Result<Self> {
        Self::parse_banner_it(input, input.split('/'))
    }

    fn parse_banner_it<'a, I>(input: &'a str, mut it: I) -> Result<Self>
    where
        I: Iterator<Item = &'a str>,
    {
        let (kind, format) = match (
            it.next(),
            it.next(),
            it.next(),
            it.next(),
            it.next(),
            it.next(),
        ) {
            (Some("images"), Some("missing"), Some(name), None, None, None) => {
                let Some(("series", ext)) = name.split_once('.') else {
                    bail!("{input}: missing extension");
                };

                let format = ImageFormat::parse(ext)?;
                (ImageKind::Missing, format)
            }
            (Some("v4"), Some("series"), Some(series_id), Some(kind), Some(name), None) => {
                let Some((id, ext)) = name.split_once('.') else {
                    bail!("{input}: missing extension");
                };

                let series_id = series_id.parse()?;
                let format = ImageFormat::parse(ext)?;
                let kind = ArtKind::parse(kind)?;
                let id = Hex16::from_hex(id).context("bad id")?;

                (
                    ImageKind::V4 {
                        series_id,
                        kind,
                        id,
                    },
                    format,
                )
            }
            (Some("series"), Some(series_id), Some(kind), Some(name), None, None) => {
                let Some((id, ext)) = name.split_once('.') else {
                    bail!("{input}: missing extension");
                };

                let series_id = series_id.parse()?;
                let format = ImageFormat::parse(ext)?;
                let kind = ArtKind::parse(kind)?;
                let id = Hex16::from_hex(id).context("bad id")?;
                (
                    ImageKind::Legacy {
                        series_id,
                        kind,
                        id,
                    },
                    format,
                )
            }
            (Some("posters"), Some(name), None, None, None, None) => {
                let Some((rest, ext)) = name.split_once('.') else {
                    bail!("{input}: missing extension");
                };

                let format = ImageFormat::parse(ext)?;

                let kind = if let Some((series_id, suffix)) = rest.split_once('-') {
                    let series_id = series_id.parse()?;
                    let suffix = Raw16::from_string(suffix);
                    ImageKind::BannerSuffixed { series_id, suffix }
                } else {
                    let id = Hex16::from_hex(rest).context("bad id")?;
                    ImageKind::Banner { id }
                };

                (kind, format)
            }
            (Some("graphical"), Some(name), None, None, None, None) => {
                let Some((rest, ext)) = name.split_once('.') else {
                    bail!("{input}: missing extension");
                };

                let Some((series_id, suffix)) = rest.split_once('-') else {
                    bail!("{input}: missing suffix");
                };

                let series_id = series_id.parse()?;
                let suffix = Raw16::from_string(suffix);
                let format = ImageFormat::parse(ext)?;
                let kind = ImageKind::Graphical { series_id, suffix };
                (kind, format)
            }
            (Some("fanart"), Some("original"), Some(name), None, None, None) => {
                let Some((rest, ext)) = name.split_once('.') else {
                    bail!("{input}: missing extension");
                };

                let Some((series_id, suffix)) = rest.split_once('-') else {
                    bail!("{input}: missing number");
                };

                let series_id = series_id.parse()?;
                let suffix = Raw16::from_string(suffix);
                let format = ImageFormat::parse(ext)?;
                let kind = ImageKind::Fanart { series_id, suffix };
                (kind, format)
            }
            _ => {
                bail!("{input}: unsupported image");
            }
        };

        Ok(Image { kind, format })
    }

    /// Generate a 16-byte hash.
    pub(crate) fn hash(&self) -> u128 {
        use std::hash::Hash;
        use twox_hash::xxh3::HasherExt;

        let mut hasher = twox_hash::Xxh3Hash128::default();
        self.kind.hash(&mut hasher);
        hasher.finish_ext()
    }

    /// Get the expected image format.
    pub(crate) fn format(&self) -> ImageFormat {
        self.format
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(tag = "type")]
#[serde(rename = "kebab-case")]
pub(crate) enum ArtKind {
    /// Poster art.
    Posters,
    /// Banner art.
    Banners,
    /// Background art.
    Backgrounds,
}

impl ArtKind {
    fn parse(input: &str) -> Result<Self> {
        match input {
            "posters" => Ok(ArtKind::Posters),
            "banners" => Ok(ArtKind::Banners),
            "backgrounds" => Ok(ArtKind::Backgrounds),
            _ => {
                bail!("{input}: unsupported art kind")
            }
        }
    }
}

impl fmt::Display for ArtKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ArtKind::Posters => write!(f, "posters"),
            ArtKind::Banners => write!(f, "banners"),
            ArtKind::Backgrounds => write!(f, "backgrounds"),
        }
    }
}

/// The identifier of an image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(tag = "type")]
#[serde(rename = "kebab-case")]
pub(crate) enum ImageKind {
    Legacy {
        series_id: u64,
        kind: ArtKind,
        id: Hex16,
    },
    V4 {
        series_id: u64,
        kind: ArtKind,
        id: Hex16,
    },
    Banner {
        id: Hex16,
    },
    BannerSuffixed {
        series_id: u64,
        suffix: Raw16,
    },
    Graphical {
        series_id: u64,
        suffix: Raw16,
    },
    Fanart {
        series_id: u64,
        suffix: Raw16,
    },
    Missing,
}

impl fmt::Display for Image {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let format = &self.format;

        match self.kind {
            ImageKind::Legacy {
                series_id,
                kind,
                id,
            } => {
                write!(f, "/banners/series/{series_id}/{kind}/{id}.{format}")
            }
            ImageKind::V4 {
                series_id,
                kind,
                id,
            } => {
                write!(f, "/banners/v4/series/{series_id}/{kind}/{id}.{format}")
            }
            ImageKind::Banner { id } => {
                write!(f, "/banners/posters/{id}.{format}")
            }
            ImageKind::BannerSuffixed { series_id, suffix } => {
                write!(f, "/banners/posters/{series_id}-{suffix}.{format}")
            }
            ImageKind::Graphical { series_id, suffix } => {
                write!(f, "/banners/graphical/{series_id}-{suffix}.{format}")
            }
            ImageKind::Fanart { series_id, suffix } => {
                write!(f, "/banners/fanart/original/{series_id}-{suffix}.{format}")
            }
            ImageKind::Missing => {
                write!(f, "/banners/images/missing/series.jpg")
            }
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SearchSeries {
    pub(crate) id: TheTvDbSeriesId,
    pub(crate) name: String,
    pub(crate) poster: Image,
    pub(crate) overview: Option<String>,
}
