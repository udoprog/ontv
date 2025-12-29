use core::error::Error;
use core::fmt;
use core::str::FromStr;

use musli_core::{Decode, Encode};
use relative_path::RelativePath;
use serde::de;
use serde::ser;
use serde::{Deserialize, Serialize};

/// Image format in use.
#[derive(
    Default,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Deserialize,
    Serialize,
    Encode,
    Decode,
)]
#[serde(rename_all = "kebab-case")]
#[musli(crate = musli_core, name_all = "kebab-case")]
pub enum ImageExt {
    Jpg,
    /// Unsupported extension.
    #[default]
    Unsupported,
}

impl fmt::Display for ImageExt {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImageExt::Jpg => write!(f, "jpg"),
            ImageExt::Unsupported => write!(f, "unsupported"),
        }
    }
}

/// The hash of an image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct ImageHash([u8; 16]);

impl fmt::Display for ImageHash {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use base64::display::Base64Display;
        use base64::engine::general_purpose::URL_SAFE_NO_PAD;
        Base64Display::new(&self.0, &URL_SAFE_NO_PAD).fmt(f)
    }
}

/// The identifier of an image.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode)]
#[non_exhaustive]
#[musli(crate = musli_core)]
pub enum ImageV2 {
    /// An image from thetvdb.com
    Tvdb {
        #[musli(with = musli::serde)]
        uri: Box<RelativePath>,
    },
    /// An image from themoviedb.org
    Tmdb {
        #[musli(with = musli::serde)]
        uri: Box<RelativePath>,
    },
}

impl ImageV2 {
    /// Generate an image hash.
    pub fn hash(&self) -> ImageHash {
        let hash = match self {
            ImageV2::Tvdb { uri } => crate::cache::hash16(&(0xd410b8f4u32, uri)),
            ImageV2::Tmdb { uri } => crate::cache::hash16(&(0xc66bff3eu32, uri)),
        };

        ImageHash(hash)
    }

    /// Construct a new tvbd image.
    pub fn tvdb(string: &(impl ?Sized + AsRef<str>)) -> Option<Self> {
        Some(string.as_ref().trim_start_matches('/'))
            .filter(|s| !s.is_empty())
            .map(|uri| Self::Tvdb { uri: uri.into() })
    }

    /// Construct a new tmdb image.
    pub fn tmdb(string: &(impl ?Sized + AsRef<str>)) -> Option<Self> {
        Some(string.as_ref().trim_start_matches('/'))
            .filter(|s| !s.is_empty())
            .map(|uri| Self::Tmdb { uri: uri.into() })
    }
}

impl fmt::Display for ImageV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImageV2::Tvdb { uri } => write!(f, "tvdb:{uri}"),
            ImageV2::Tmdb { uri } => write!(f, "tmdb:{uri}"),
        }
    }
}

/// An error raised when parsing an image v2 uri.
pub struct ImageV2Err {
    kind: ImageV2ErrKind,
}

impl fmt::Debug for ImageV2Err {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.kind.fmt(f)
    }
}

impl fmt::Display for ImageV2Err {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.kind.fmt(f)
    }
}

impl From<ImageV2ErrKind> for ImageV2Err {
    #[inline]
    fn from(kind: ImageV2ErrKind) -> Self {
        Self { kind }
    }
}

#[derive(Debug)]
enum ImageV2ErrKind {
    MissingSeparator,
    InvalidKind,
}

impl fmt::Display for ImageV2ErrKind {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSeparator => write!(f, "missing `:` separator"),
            Self::InvalidKind => write!(f, "invalid image v2 kind"),
        }
    }
}

impl FromStr for ImageV2 {
    type Err = ImageV2Err;

    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use ImageV2ErrKind::*;

        let (head, uri) = s.split_once(':').ok_or(MissingSeparator)?;

        match head {
            "tmdb" => Ok(ImageV2::Tmdb { uri: uri.into() }),
            "tvdb" => Ok(ImageV2::Tvdb { uri: uri.into() }),
            _ => Err(ImageV2Err::from(InvalidKind)),
        }
    }
}

impl Serialize for ImageV2 {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for ImageV2 {
    #[inline]
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct ImageV2Visitor;

        impl<'de> de::Visitor<'de> for ImageV2Visitor {
            type Value = ImageV2;

            #[inline]
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "an image v2 uri")
            }

            #[inline]
            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                ImageV2::from_str(v).map_err(de::Error::custom)
            }
        }

        deserializer.deserialize_str(ImageV2Visitor)
    }
}

/// Season number.
#[derive(
    Default,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    Encode,
    Decode,
)]
#[serde(untagged)]
#[musli(crate = musli_core)]
#[musli(untagged)]
pub enum SeasonNumber {
    /// Season used for non-numbered episodes.
    #[default]
    Specials,
    /// A regular numbered season.
    Number(u32),
}

impl SeasonNumber {
    #[inline]
    pub fn is_special(&self) -> bool {
        matches!(self, SeasonNumber::Specials)
    }

    /// Build season title.
    pub fn short(&self) -> SeasonShort<'_> {
        SeasonShort { season: self }
    }
}

impl fmt::Display for SeasonNumber {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SeasonNumber::Specials => write!(f, "Specials"),
            SeasonNumber::Number(number) => write!(f, "Season {number}"),
        }
    }
}

/// Short season number display.
pub struct SeasonShort<'a> {
    season: &'a SeasonNumber,
}

impl fmt::Display for SeasonShort<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.season {
            SeasonNumber::Specials => "S".fmt(f),
            SeasonNumber::Number(n) => n.fmt(f),
        }
    }
}

/// How to resize an image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImageSizeHint {
    Fit(u32, u32),
    Fill(u32, u32),
}

impl fmt::Display for ImageSizeHint {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImageSizeHint::Fit(w, h) => write!(f, "fit-{}x{}", w, h),
            ImageSizeHint::Fill(w, h) => write!(f, "fill-{}x{}", w, h),
        }
    }
}

impl FromStr for ImageSizeHint {
    type Err = ImageHintErr;

    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use ImageHintErrKind::*;

        let (kind, tail) = s.split_once('-').ok_or(MissingKindSeparator)?;
        let (w, h) = tail.split_once('x').ok_or(MissingDimensionSeparator)?;
        let w = w.parse::<u32>().map_err(|_| InvalidWidth)?;
        let h = h.parse::<u32>().map_err(|_| InvalidHeight)?;

        match kind {
            "fit" => Ok(Self::Fit(w, h)),
            "fill" => Ok(Self::Fill(w, h)),
            _ => Err(ImageHintErr::from(ImageHintErrKind::InvalidKind)),
        }
    }
}

/// Whether or not to provide a scaled version of the image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImageHint {
    /// Resize an image.
    Resize(ImageSizeHint),
    /// Original image,
    Original,
}

impl fmt::Display for ImageHint {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImageHint::Resize(hint) => hint.fmt(f),
            ImageHint::Original => write!(f, "original"),
        }
    }
}

pub struct ImageHintErr {
    kind: ImageHintErrKind,
}

impl fmt::Debug for ImageHintErr {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.kind.fmt(f)
    }
}

impl fmt::Display for ImageHintErr {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.kind.fmt(f)
    }
}

impl From<ImageHintErrKind> for ImageHintErr {
    #[inline]
    fn from(kind: ImageHintErrKind) -> Self {
        Self { kind }
    }
}

impl Error for ImageHintErr {}

#[derive(Debug)]
enum ImageHintErrKind {
    MissingKindSeparator,
    MissingDimensionSeparator,
    InvalidWidth,
    InvalidHeight,
    InvalidKind,
}

impl fmt::Display for ImageHintErrKind {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingKindSeparator => write!(f, "missing kind separator `-`"),
            Self::MissingDimensionSeparator => write!(f, "missing dimension separator `x`"),
            Self::InvalidWidth => write!(f, "invalid image hint width"),
            Self::InvalidHeight => write!(f, "invalid image hint height"),
            Self::InvalidKind => write!(
                f,
                "invalid image hint kind, expected one of `fit` or `fill`"
            ),
        }
    }
}

impl FromStr for ImageHint {
    type Err = ImageHintErr;

    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "original" => Ok(ImageHint::Original),
            s => Ok(ImageHint::Resize(s.parse()?)),
        }
    }
}

impl<'de> Deserialize<'de> for ImageHint {
    #[inline]
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = ImageHint;

            #[inline]
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "an image hint string")
            }

            #[inline]
            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                ImageHint::from_str(v).map_err(de::Error::custom)
            }
        }

        deserializer.deserialize_str(Visitor)
    }
}
