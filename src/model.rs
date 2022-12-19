mod hex16;
mod raw16;

#[cfg(test)]
mod tests;

use std::fmt;

use anyhow::{bail, ensure, Context, Result};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub(crate) use self::hex16::Hex16;
pub(crate) use self::raw16::Raw16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "type")]
pub(crate) enum RemoteId {
    Series {
        #[serde(flatten)]
        id: RemoteSeriesId,
    },
    Episode {
        #[serde(flatten)]
        id: RemoteEpisodeId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "remote")]
pub(crate) enum RemoteSeriesId {
    TheTvDb { id: SeriesId },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "remote")]
pub(crate) enum RemoteEpisodeId {
    TheTvDb { id: SeriesId },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(transparent)]
#[serde(transparent)]
pub(crate) struct SeriesId(u64);

impl From<u64> for SeriesId {
    #[inline]
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl fmt::Display for SeriesId {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// A series.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct Series {
    /// Allocated UUID.
    pub(crate) id: Uuid,
    /// Title of the series.
    pub(crate) title: String,
    /// Poster image.
    pub(crate) poster: Image,
    /// Banner image.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) banner: Option<Image>,
    /// Fanart image.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) fanart: Option<Image>,
    /// Remote series ids.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) remote_ids: Vec<RemoteSeriesId>,
    /// Indicates if the series is tracked or not, in that it will receive updates.
    #[serde(default, skip_serializing_if = "is_false")]
    pub(crate) tracked: bool,
}

#[inline]
fn is_false(b: &bool) -> bool {
    !*b
}

/// A season in a series.
#[derive(Default, Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct Watched {
    /// Identifier of watched episode.
    pub(crate) episode: Uuid,
    pub(crate) timestamp: DateTime<Utc>,
}

/// A season in a series.
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Season {
    /// The number of the season.
    pub(crate) number: Option<u32>,
}

/// An episode in a series.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Episode {
    /// Uuid of the watched episode.
    pub(crate) id: Uuid,
    /// Name of the episode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) name: Option<String>,
    /// Overview of the episode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) overview: Option<String>,
    /// Absolute number in the series.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) absolute_number: Option<u32>,
    /// Season number. If empty indicates special season.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) season: Option<u32>,
    /// Number in the season.
    pub(crate) number: u32,
    /// Air date of the episode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) aired: Option<NaiveDate>,
    /// Episode image.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) filename: Option<Image>,
    /// Remote episode ids.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) remote_ids: Vec<RemoteEpisodeId>,
}

/// Image format in use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
pub(crate) struct Image {
    #[serde(flatten)]
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
                (ImageKind::V4(series_id, kind, id), format)
            }
            (Some("series"), Some(series_id), Some(kind), Some(name), None, None) => {
                let Some((id, ext)) = name.split_once('.') else {
                    bail!("{input}: missing extension");
                };

                let series_id = series_id.parse()?;
                let format = ImageFormat::parse(ext)?;
                let kind = ArtKind::parse(kind)?;
                let id = Hex16::from_hex(id).context("bad id")?;
                (ImageKind::Legacy(series_id, kind, id), format)
            }
            (Some("posters"), Some(name), None, None, None, None) => {
                let Some((rest, ext)) = name.split_once('.') else {
                    bail!("{input}: missing extension");
                };

                let format = ImageFormat::parse(ext)?;

                let kind = if let Some((series_id, suffix)) = rest.split_once('-') {
                    let series_id = series_id.parse()?;
                    let suffix = Raw16::from_string(suffix);
                    ImageKind::BannerSuffixed(series_id, suffix)
                } else {
                    let id = Hex16::from_hex(rest).context("bad id")?;
                    ImageKind::Banner(id)
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
                let kind = ImageKind::Graphical(series_id, suffix);
                (kind, format)
            }
            (Some("fanart"), Some("original"), Some(name), None, None, None) => {
                let Some((rest, ext)) = name.split_once('.') else {
                    bail!("{input}: missing extension");
                };

                let format = ImageFormat::parse(ext)?;

                let kind = if let Some((series_id, suffix)) = rest.split_once('-') {
                    let series_id = series_id.parse()?;
                    let suffix = Raw16::from_string(suffix);
                    ImageKind::FanartSuffixed(series_id, suffix)
                } else {
                    let id = Hex16::from_hex(rest).context("bad hex")?;
                    ImageKind::Fanart(id)
                };

                (kind, format)
            }
            // Example: v4/episode/8538342/screencap/63887bf74c84e.jpg
            (
                Some("v4"),
                Some("episode"),
                Some(episode_id),
                Some("screencap"),
                Some(name),
                None,
            ) => {
                let Some((name, ext)) = name.split_once('.') else {
                    bail!("{input}: missing extension");
                };

                let format = ImageFormat::parse(ext)?;
                let episode_id = episode_id.parse()?;
                let id = Hex16::from_hex(name).context("bad id")?;
                let kind = ImageKind::ScreenCap(episode_id, id);
                (kind, format)
            }
            (Some("episodes"), Some(episode_id), Some(name), None, None, None) => {
                let Some((image_id, ext)) = name.split_once('.') else {
                    bail!("{input}: missing extension");
                };

                let format = ImageFormat::parse(ext)?;
                let episode_id = episode_id.parse()?;
                let image_id = image_id.parse()?;
                let kind = ImageKind::Episodes(episode_id, image_id);
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ArtKind {
    /// Poster art.
    Posters,
    /// Banner art.
    Banners,
    /// Background art.
    Backgrounds,
    /// Episodes art.
    Episodes,
}

impl ArtKind {
    fn parse(input: &str) -> Result<Self> {
        match input {
            "posters" => Ok(ArtKind::Posters),
            "banners" => Ok(ArtKind::Banners),
            "backgrounds" => Ok(ArtKind::Backgrounds),
            "episodes" => Ok(ArtKind::Episodes),
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
            ArtKind::Episodes => write!(f, "episodes"),
        }
    }
}

/// The identifier of an image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", tag = "type", content = "data")]
pub(crate) enum ImageKind {
    Legacy(u64, ArtKind, Hex16),
    V4(u64, ArtKind, Hex16),
    Banner(Hex16),
    BannerSuffixed(u64, Raw16),
    Graphical(u64, Raw16),
    Fanart(Hex16),
    FanartSuffixed(u64, Raw16),
    ScreenCap(u64, Hex16),
    Episodes(u32, u32),
    Missing,
}

impl fmt::Display for Image {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let format = &self.format;

        match self.kind {
            ImageKind::Legacy(series_id, kind, id) => {
                write!(f, "/banners/series/{series_id}/{kind}/{id}.{format}")
            }
            ImageKind::V4(series_id, kind, id) => {
                write!(f, "/banners/v4/series/{series_id}/{kind}/{id}.{format}")
            }
            ImageKind::Banner(id) => {
                write!(f, "/banners/posters/{id}.{format}")
            }
            ImageKind::BannerSuffixed(series_id, suffix) => {
                write!(f, "/banners/posters/{series_id}-{suffix}.{format}")
            }
            ImageKind::Graphical(series_id, suffix) => {
                write!(f, "/banners/graphical/{series_id}-{suffix}.{format}")
            }
            ImageKind::Fanart(id) => {
                write!(f, "/banners/fanart/original/{id}.{format}")
            }
            ImageKind::FanartSuffixed(series_id, suffix) => {
                write!(f, "/banners/fanart/original/{series_id}-{suffix}.{format}")
            }
            ImageKind::ScreenCap(episode_id, id) => {
                write!(
                    f,
                    "/banners/v4/episode/{episode_id}/screencap/{id}.{format}"
                )
            }
            ImageKind::Episodes(episode_id, image_id) => {
                write!(f, "/banners/episodes/{episode_id}/{image_id}.{format}")
            }
            ImageKind::Missing => {
                write!(f, "/banners/images/missing/series.jpg")
            }
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SearchSeries {
    pub(crate) id: SeriesId,
    pub(crate) name: String,
    pub(crate) poster: Image,
    pub(crate) overview: Option<String>,
}
