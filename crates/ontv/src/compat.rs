//! Module containing types which are only used for migrating old database structures.
//!
//! These will be deprecated as new major releases of ontv are released.

mod hex;

use core::fmt;

use relative_path::RelativePathBuf;
use serde::{Deserialize, Serialize};

use crate::model::{ImageExt, ImageV2, Raw};

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
pub(crate) enum TvdbImageKind {
    Legacy(u64, ArtKind, hex::Hex<16>),
    V4(u64, ArtKind, hex::Hex<16>),
    Banner(hex::Hex<16>),
    BannerSuffixed(u64, Raw<16>),
    Graphical(hex::Hex<16>),
    GraphicalSuffixed(u64, Raw<16>),
    Fanart(hex::Hex<16>),
    FanartSuffixed(u64, Raw<16>),
    ScreenCap(u64, hex::Hex<16>),
    Episodes(u32, u32),
    Blank(u32),
    Text(u32),
    Missing,
}

/// An image from thetvdb.com.org
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct TvdbImage {
    #[serde(flatten)]
    pub(crate) kind: TvdbImageKind,
    pub(crate) ext: ImageExt,
}

impl fmt::Display for TvdbImage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ext = &self.ext;

        match &self.kind {
            TvdbImageKind::Legacy(series_id, kind, id) => {
                write!(f, "series/{series_id}/{kind}/{id}.{ext}")
            }
            TvdbImageKind::V4(series_id, kind, id) => {
                write!(f, "v4/series/{series_id}/{kind}/{id}.{ext}")
            }
            TvdbImageKind::Banner(id) => {
                write!(f, "posters/{id}.{ext}")
            }
            TvdbImageKind::BannerSuffixed(series_id, suffix) => {
                write!(f, "posters/{series_id}-{suffix}.{ext}")
            }
            TvdbImageKind::Graphical(id) => {
                write!(f, "graphical/{id}.{ext}")
            }
            TvdbImageKind::GraphicalSuffixed(series_id, suffix) => {
                write!(f, "graphical/{series_id}-{suffix}.{ext}")
            }
            TvdbImageKind::Fanart(id) => {
                write!(f, "fanart/original/{id}.{ext}")
            }
            TvdbImageKind::FanartSuffixed(series_id, suffix) => {
                write!(f, "fanart/original/{series_id}-{suffix}.{ext}")
            }
            TvdbImageKind::ScreenCap(episode_id, id) => {
                write!(f, "v4/episode/{episode_id}/screencap/{id}.{ext}")
            }
            TvdbImageKind::Episodes(episode_id, image_id) => {
                write!(f, "episodes/{episode_id}/{image_id}.{ext}")
            }
            TvdbImageKind::Blank(series_id) => {
                write!(f, "blank/{series_id}.{ext}")
            }
            TvdbImageKind::Text(series_id) => {
                write!(f, "text/{series_id}.{ext}")
            }
            TvdbImageKind::Missing => {
                write!(f, "images/missing/series.{ext}")
            }
        }
    }
}

/// The identifier of an image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", tag = "type", content = "data")]
pub(crate) enum TmdbImageKind {
    Base64(Raw<32>),
}

/// An image from themoviedb.org
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct TmdbImage {
    #[serde(flatten)]
    pub(crate) kind: TmdbImageKind,
    pub(crate) ext: ImageExt,
}

impl fmt::Display for TmdbImage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ext = &self.ext;

        match self.kind {
            TmdbImageKind::Base64(id) => {
                write!(f, "{id}.{ext}")?;
            }
        }

        Ok(())
    }
}

/// The identifier of an image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", tag = "from")]
pub(crate) enum Image {
    /// An image from thetvdb.com
    Tvdb(TvdbImage),
    /// An image from themoviedb.org
    Tmdb(TmdbImage),
}

impl Image {
    /// Convert into a V2 image.
    pub(crate) fn into_v2(self) -> ImageV2 {
        match self {
            Image::Tvdb(image) => ImageV2::Tvdb {
                uri: RelativePathBuf::from(image.to_string()).into(),
            },
            Image::Tmdb(image) => ImageV2::Tmdb {
                uri: RelativePathBuf::from(image.to_string()).into(),
            },
        }
    }
}

impl fmt::Display for Image {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Image::Tvdb(image) => write!(f, "tvdb:{image}"),
            Image::Tmdb(image) => write!(f, "tmdb:{image}"),
        }
    }
}

impl From<TvdbImage> for Image {
    #[inline]
    fn from(image: TvdbImage) -> Self {
        Image::Tvdb(image)
    }
}

impl From<TmdbImage> for Image {
    #[inline]
    fn from(image: TmdbImage) -> Self {
        Image::Tmdb(image)
    }
}
