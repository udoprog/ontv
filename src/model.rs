#[cfg(test)]
mod tests;

use std::fmt;
use std::sync::Arc;

use anyhow::{bail, ensure, Context, Result};
use serde::de;
use serde::ser;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct Id([u8; 16]);

impl Id {
    /// Construct an ID from bytes.
    pub(crate) fn from_hex<B>(bytes: &B) -> Option<Self>
    where
        B: ?Sized + AsRef<[u8]>,
    {
        let mut out = [0; 16];

        for (&b, to) in bytes.as_ref().iter().rev().zip((0..=31).rev()) {
            let (base, add) = match b {
                b'0'..=b'9' => (b'0', 0),
                b'a'..=b'f' => (b'a', 0xa),
                b'A'..=b'F' => (b'A', 0xa),
                _ => return None,
            };

            out[to / 2] |= (b - base + add) << 4 * u8::from(to % 2 == 0);
        }

        Some(Self(out))
    }
}

impl From<u128> for Id {
    #[inline]
    fn from(value: u128) -> Self {
        Self(value.to_be_bytes())
    }
}

impl fmt::Display for Id {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for b in self
            .0
            .iter()
            .flat_map(|b| [*b >> 4, *b & 0b1111])
            .skip_while(|b| *b == 0)
        {
            let b = match b {
                0xa..=0xf => b'a' + (b - 0xa),
                _ => b'0' + b,
            };

            (b as char).fmt(f)?;
        }

        Ok(())
    }
}

impl<'de> de::Deserialize<'de> for Id {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        return deserializer.deserialize_bytes(Visitor);

        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = Id;

            #[inline]
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "an identifier")
            }

            #[inline]
            fn visit_borrowed_str<E>(self, v: &'de str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                match Id::from_hex(v) {
                    Some(id) => Ok(id),
                    None => Err(E::custom("bad identifier")),
                }
            }

            #[inline]
            fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                match Id::from_hex(v) {
                    Some(id) => Ok(id),
                    None => Err(E::custom("bad identifier")),
                }
            }
        }
    }
}

impl ser::Serialize for Id {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_str(self)
    }
}

/// A single episode in a series.
#[derive(Debug, Clone)]
pub(crate) struct Episode {
    title: Arc<str>,
    season: u32,
    number: u32,
    series: Id,
}

/// A series.
#[derive(Debug, Clone)]
pub(crate) struct Series {
    title: Arc<str>,
    cover: Id,
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

/// The identifier of an image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(tag = "type")]
#[serde(rename = "kebab-case")]
pub(crate) enum Image {
    Series {
        series_id: u64,
        id: Id,
        format: ImageFormat,
    },
    SeriesV4 {
        series_id: u64,
        id: Id,
        format: ImageFormat,
    },
    Banner {
        id: Id,
        format: ImageFormat,
    },
    BannerNumbered {
        series_id: u64,
        number: u32,
        format: ImageFormat,
    },
    Missing,
}

impl fmt::Display for Image {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Image::Series {
                series_id,
                id,
                format,
            } => {
                write!(f, "/banners/series/{series_id}/posters/{id}.{format}")
            }
            Image::SeriesV4 {
                series_id,
                id,
                format,
            } => {
                write!(f, "/banners/v4/series/{series_id}/posters/{id}.{format}")
            }
            Image::Banner { id, format } => {
                write!(f, "/banners/posters/{id}.{format}")
            }
            Image::BannerNumbered {
                series_id,
                number,
                format,
            } => {
                write!(f, "/banners/posters/{series_id}-{number}.{format}")
            }
            Image::Missing => {
                write!(f, "/banners/images/missing/series.jpg")
            }
        }
    }
}

impl Image {
    /// Parse an image URL from thetvdb.
    pub(crate) fn thetvdb_parse(input: &str) -> Result<Self> {
        if input == "/banners/images/missing/series.jpg" {
            return Ok(Image::Missing);
        }

        let mut it = input.split('/');
        ensure!(it.next().is_some(), "{input}: missing leading");
        ensure!(
            matches!(it.next(), Some("banners")),
            "{input}: missing `banners`"
        );

        match (
            it.next(),
            it.next(),
            it.next(),
            it.next(),
            it.next(),
            it.next(),
        ) {
            (Some("v4"), Some("series"), Some(series_id), Some("posters"), Some(name), None) => {
                let Some((id, ext)) = name.split_once('.') else {
                    bail!("{input}: missing extension");
                };

                let series_id = series_id.parse()?;
                let format = ImageFormat::parse(ext)?;
                let id = Id::from_hex(id).context("bad id")?;

                Ok(Image::SeriesV4 {
                    series_id,
                    id,
                    format,
                })
            }
            (Some("series"), Some(series_id), Some("posters"), Some(name), None, None) => {
                let Some((id, ext)) = name.split_once('.') else {
                    bail!("{input}: missing extension");
                };

                let series_id = series_id.parse()?;
                let format = ImageFormat::parse(ext)?;
                let id = Id::from_hex(id).context("bad id")?;

                Ok(Image::Series {
                    series_id,
                    id,
                    format,
                })
            }
            (Some("posters"), Some(name), None, None, None, None) => {
                let Some((rest, ext)) = name.split_once('.') else {
                    bail!("{input}: missing extension");
                };

                let format = ImageFormat::parse(ext)?;

                if let Some((series_id, number)) = rest.split_once('-') {
                    let series_id = series_id.parse()?;
                    let number = number.parse()?;

                    Ok(Image::BannerNumbered {
                        series_id,
                        number,
                        format,
                    })
                } else {
                    let id = Id::from_hex(rest).context("bad id")?;
                    Ok(Image::Banner { id, format })
                }
            }
            _ => {
                bail!("{input}: unsupported image");
            }
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SearchSeries {
    pub(crate) name: String,
    pub(crate) poster: Image,
    pub(crate) overview: String,
}
