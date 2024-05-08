use std::collections::btree_set::BTreeSet;
use std::fmt;
use std::str::FromStr;

use chrono::{DateTime, NaiveDate, Utc};
use musli::{Decode, Encode};
use relative_path::RelativePath;
use serde::de::IntoDeserializer;
use serde::{de, ser};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{EpisodeId, MovieId, Raw, SeriesId, WatchedId};

/// Whether or not to provide a scaled version of the image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImageHint {
    /// No hint.
    Raw,
    /// Specifies that the image should fit centered within the specified bounds.
    Fit(u32, u32),
    /// Fill the specified dimensions.
    Fill(u32, u32),
}

impl fmt::Display for ImageHint {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImageHint::Raw => write!(f, ""),
            ImageHint::Fit(w, h) => write!(f, "-fit-{w}x{h}"),
            ImageHint::Fill(w, h) => write!(f, "-fill-{w}x{h}"),
        }
    }
}

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

/// A series that is scheduled to be aired.
#[derive(Clone, Encode, Decode)]
pub struct ScheduledSeries {
    pub series_id: SeriesId,
    pub episodes: Vec<EpisodeId>,
}

/// A scheduled day.
#[derive(Clone, Encode, Decode)]
pub struct ScheduledDay {
    #[musli(with = musli::serde)]
    pub date: NaiveDate,
    pub schedule: Vec<ScheduledSeries>,
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

impl fmt::Display for SeasonNumber {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SeasonNumber::Specials => write!(f, "Specials"),
            SeasonNumber::Number(number) => write!(f, "Season {number}"),
        }
    }
}

/// Associated season graphics.
#[derive(Default, Debug, Clone, Serialize, Deserialize, Encode, Decode)]
#[serde(rename_all = "snake_case")]
pub struct SeasonGraphics {
    /// Poster for season.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[musli(default, skip_encoding_if = Option::is_none)]
    pub poster: Option<ImageV2>,
}

impl SeasonGraphics {
    fn is_empty(&self) -> bool {
        self.poster.is_none()
    }
}

/// A season in a series.
#[derive(Default, Debug, Clone, Serialize, Deserialize, Encode, Decode)]
#[serde(rename_all = "snake_case")]
pub struct Season {
    /// The number of the season.
    #[serde(default, skip_serializing_if = "SeasonNumber::is_special")]
    #[musli(default, skip_encoding_if = SeasonNumber::is_special)]
    pub number: SeasonNumber,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[musli(default, skip_encoding_if = Option::is_none, with = musli::serde)]
    pub air_date: Option<NaiveDate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[musli(default, skip_encoding_if = Option::is_none)]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[musli(default, skip_encoding_if = String::is_empty)]
    pub overview: String,
    #[serde(default, skip_serializing_if = "SeasonGraphics::is_empty")]
    #[musli(default, skip_encoding_if = SeasonGraphics::is_empty)]
    pub graphics: SeasonGraphics,
}

impl Season {
    /// Get the poster of the season.
    pub fn poster(&self) -> Option<&ImageV2> {
        self.graphics.poster.as_ref()
    }
}

/// Associated episode graphics.
#[derive(Default, Debug, Clone, Serialize, Deserialize, Encode, Decode)]
#[serde(rename_all = "snake_case")]
pub struct EpisodeGraphics {
    /// Filename for episode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filename: Option<ImageV2>,
}

impl EpisodeGraphics {
    fn is_empty(&self) -> bool {
        self.filename.is_none()
    }
}

/// An episode in a series.
#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
#[serde(rename_all = "snake_case")]
pub struct Episode {
    /// Uuid of the watched episode.
    pub id: EpisodeId,
    /// Name of the episode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[musli(default, skip_encoding_if = Option::is_none)]
    pub name: Option<String>,
    /// Overview of the episode.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[musli(default, skip_encoding_if = String::is_empty)]
    pub overview: String,
    /// Absolute number in the series.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[musli(default, skip_encoding_if = Option::is_none)]
    pub absolute_number: Option<u32>,
    /// Season number.
    #[serde(default, skip_serializing_if = "SeasonNumber::is_special")]
    #[musli(default, skip_encoding_if = SeasonNumber::is_special)]
    pub season: SeasonNumber,
    /// Episode number inside of its season.
    pub number: u32,
    /// Air date of the episode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[musli(default, skip_encoding_if = Option::is_none, with = musli::serde)]
    pub aired: Option<NaiveDate>,
    /// Episode graphics.
    #[serde(default, skip_serializing_if = "EpisodeGraphics::is_empty")]
    #[musli(default, skip_encoding_if = EpisodeGraphics::is_empty)]
    pub graphics: EpisodeGraphics,
    /// The remote identifier that is used to synchronize this episode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[musli(default, skip_encoding_if = Option::is_none)]
    pub remote_id: Option<RemoteEpisodeId>,
}

impl Episode {
    /// Get filename for episode.
    pub fn filename(&self) -> Option<&ImageV2> {
        self.graphics.filename.as_ref()
    }

    /// Test if episode will air in the future.
    ///
    /// This ignores episodes without an air date.
    pub fn will_air(&self, today: &NaiveDate) -> bool {
        let Some(aired) = self.aired else {
            return false;
        };

        aired > *today
    }

    /// Test if the given episode has aired by the provided timestamp.
    pub fn has_aired(&self, today: &NaiveDate) -> bool {
        let Some(aired) = self.aired else {
            return false;
        };

        aired <= *today
    }

    /// Get aired timestamp.
    pub fn aired_timestamp(&self) -> Option<DateTime<Utc>> {
        self.aired.as_ref().and_then(|&d| {
            Some(DateTime::from_naive_utc_and_offset(
                d.and_hms_opt(0, 0, 0)?,
                Utc,
            ))
        })
    }

    /// A sort key used for episodes.
    pub fn watch_order_key(&self) -> WatchOrderKey {
        WatchOrderKey {
            absolute_number: self.absolute_number.unwrap_or(u32::MAX),
            aired: self.aired.unwrap_or(NaiveDate::MAX),
            season: self.season,
            number: self.number,
        }
    }
}

impl fmt::Display for Episode {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} / {}", self.season, self.number)
    }
}

/// A key used to sort episodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WatchOrderKey {
    /// Absolute number in the series.
    pub absolute_number: u32,
    /// Air date of the episode.
    pub aired: NaiveDate,
    /// Season number.
    pub season: SeasonNumber,
    /// Episode number inside of its season.
    pub number: u32,
}

/// The identifier of an image.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode)]
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
    /// Construct a local URL for the image.
    pub fn url(&self, hint: ImageHint) -> String {
        const BASE: &str = "/api/graphics";

        match hint {
            ImageHint::Raw => format!("{BASE}/{self}"),
            ImageHint::Fit(width, height) => {
                format!("{BASE}/{self}?hint=fit&width={width}&height={height}")
            }
            ImageHint::Fill(width, height) => {
                format!("{BASE}/{self}?hint=fill&width={width}&height={height}")
            }
        }
    }

    /// Generate an image hash.
    pub fn hash(&self) -> ImageHash {
        ImageHash(match self {
            ImageV2::Tvdb { uri } => hash128(&(0xd410b8f4u32, uri)),
            ImageV2::Tmdb { uri } => hash128(&(0xc66bff3eu32, uri)),
        })
    }

    /// Construct a new tvbd image.
    pub fn tvdb<S>(string: &S) -> Option<Self>
    where
        S: ?Sized + AsRef<str>,
    {
        Some(string.as_ref().trim_start_matches('/'))
            .filter(|s| !s.is_empty())
            .map(|uri| Self::Tvdb { uri: uri.into() })
    }

    /// Construct a new tmdb image.
    pub fn tmdb<S>(string: &S) -> Option<Self>
    where
        S: ?Sized + AsRef<str>,
    {
        Some(string.as_ref().trim_start_matches('/'))
            .filter(|s| !s.is_empty())
            .map(|uri| Self::Tmdb { uri: uri.into() })
    }
}

#[derive(Debug)]
pub enum ImageParseError {
    MissingKind,
    BadKind,
}

impl fmt::Display for ImageParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImageParseError::MissingKind => write!(f, "missing kind"),
            ImageParseError::BadKind => write!(f, "bad kind"),
        }
    }
}

impl core::error::Error for ImageParseError {}

impl FromStr for ImageV2 {
    type Err = ImageParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (head, uri) = s.split_once(':').ok_or(ImageParseError::MissingKind)?;

        match head {
            "tmdb" => Ok(ImageV2::Tmdb { uri: uri.into() }),
            "tvdb" => Ok(ImageV2::Tvdb { uri: uri.into() }),
            _ => Err(ImageParseError::BadKind),
        }
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

/// The hash of an image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct ImageHash(u128);

impl ImageHash {
    pub fn as_u128(&self) -> u128 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode)]
pub enum RemoteEpisodeId {
    Tvdb { id: u32 },
    Tmdb { id: u32 },
    Imdb { id: Raw<16> },
}

impl fmt::Display for RemoteEpisodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RemoteEpisodeId::Tvdb { id } => {
                write!(f, "tvdb:{id}")
            }
            RemoteEpisodeId::Tmdb { id } => {
                write!(f, "tmdb:{id}")
            }
            RemoteEpisodeId::Imdb { id } => {
                write!(f, "imdb:{id}")
            }
        }
    }
}

impl Serialize for RemoteEpisodeId {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for RemoteEpisodeId {
    #[inline]
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct RemoteEpisodeIdVisitor;

        impl<'de> de::Visitor<'de> for RemoteEpisodeIdVisitor {
            type Value = RemoteEpisodeId;

            #[inline]
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "a remote series id")
            }

            #[inline]
            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                let (head, tail) = v
                    .split_once(':')
                    .ok_or_else(|| de::Error::custom("missing `:`"))?;

                match head {
                    "tmdb" => Ok(RemoteEpisodeId::Tmdb {
                        id: tail.parse().map_err(E::custom)?,
                    }),
                    "tvdb" => Ok(RemoteEpisodeId::Tvdb {
                        id: tail.parse().map_err(E::custom)?,
                    }),
                    "imdb" => Ok(RemoteEpisodeId::Imdb {
                        id: Raw::new(tail)
                            .ok_or_else(|| de::Error::custom("overflowing imdb identifier"))?,
                    }),
                    kind => Err(de::Error::invalid_value(de::Unexpected::Str(kind), &self)),
                }
            }

            #[inline]
            fn visit_map<A>(self, mut map: A) -> std::result::Result<Self::Value, A::Error>
            where
                A: de::MapAccess<'de>,
            {
                let mut remote = None;
                let mut id = None;

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "remote" => {
                            remote = Some(map.next_value::<String>()?);
                        }
                        "id" => {
                            id = Some(map.next_value::<serde_json::Value>()?);
                        }
                        kind => {
                            return Err(de::Error::custom(format_args!("unsupported key: {kind}")));
                        }
                    }
                }

                let (Some(remote), Some(id)) = (remote, id) else {
                    return Err(de::Error::custom("missing remote or id"));
                };

                let id = id.into_deserializer();

                match remote.as_str() {
                    "tmdb" => Ok(RemoteEpisodeId::Tmdb {
                        id: u32::deserialize(id).map_err(de::Error::custom)?,
                    }),
                    "tvdb" => Ok(RemoteEpisodeId::Tvdb {
                        id: u32::deserialize(id).map_err(de::Error::custom)?,
                    }),
                    "imdb" => Ok(RemoteEpisodeId::Imdb {
                        id: Raw::deserialize(id).map_err(de::Error::custom)?,
                    }),
                    kind => Err(de::Error::invalid_value(de::Unexpected::Str(kind), &self)),
                }
            }
        }

        deserializer.deserialize_any(RemoteEpisodeIdVisitor)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "type")]
pub enum RemoteIds {
    Series {
        uuid: SeriesId,
        remotes: Vec<RemoteId>,
    },
    Movies {
        uuid: MovieId,
        remotes: Vec<RemoteId>,
    },
    Episode {
        uuid: EpisodeId,
        remotes: Vec<RemoteEpisodeId>,
    },
}

/// Remote series season.
#[derive(Debug, Clone, Copy)]
pub enum RemoteSeasonId {
    Tmdb { id: u32, season: SeasonNumber },
    Imdb { id: Raw<16>, season: SeasonNumber },
}

impl RemoteSeasonId {
    pub fn url(&self) -> String {
        match self {
            RemoteSeasonId::Tmdb { id, season } => {
                let season = match season {
                    SeasonNumber::Specials => 0,
                    SeasonNumber::Number(n) => *n,
                };

                format!("https://www.themoviedb.org/tv/{id}/season/{season}")
            }
            RemoteSeasonId::Imdb { id, season } => {
                let season = match season {
                    SeasonNumber::Specials => -1,
                    SeasonNumber::Number(n) => *n as i64,
                };

                format!("https://www.imdb.com/title/{id}/episodes?season={season}")
            }
        }
    }
}

impl fmt::Display for RemoteSeasonId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RemoteSeasonId::Tmdb { id, season } => {
                write!(f, "tmdb:{id} ({season})")
            }
            RemoteSeasonId::Imdb { id, season } => {
                write!(f, "imdb:{id} ({season})")
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode)]
pub enum RemoteId {
    Tvdb { id: u32 },
    Tmdb { id: u32 },
    Imdb { id: Raw<16> },
}

impl RemoteId {
    /// Coerce into a remote season.
    pub fn into_season(self, season: SeasonNumber) -> Option<RemoteSeasonId> {
        match self {
            RemoteId::Tmdb { id } => Some(RemoteSeasonId::Tmdb { id, season }),
            RemoteId::Imdb { id } => Some(RemoteSeasonId::Imdb { id, season }),
            _ => None,
        }
    }

    pub fn url(&self) -> String {
        match self {
            RemoteId::Tvdb { id } => {
                format!("https://thetvdb.com/search?query={id}")
            }
            RemoteId::Tmdb { id } => {
                format!("https://www.themoviedb.org/tv/{id}")
            }
            RemoteId::Imdb { id } => {
                format!("https://www.imdb.com/title/{id}/")
            }
        }
    }

    /// Test if the remote is supported for syncing.
    pub fn is_supported(&self) -> bool {
        matches!(self, RemoteId::Tmdb { .. } | RemoteId::Tvdb { .. })
    }
}

impl fmt::Display for RemoteId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RemoteId::Tvdb { id } => {
                write!(f, "tvdb:{id}")
            }
            RemoteId::Tmdb { id } => {
                write!(f, "tmdb:{id}")
            }
            RemoteId::Imdb { id } => {
                write!(f, "imdb:{id}")
            }
        }
    }
}

impl Serialize for RemoteId {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for RemoteId {
    #[inline]
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct RemoteIdVisitor;

        impl<'de> de::Visitor<'de> for RemoteIdVisitor {
            type Value = RemoteId;

            #[inline]
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "a remote series id")
            }

            #[inline]
            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                let (head, tail) = v
                    .split_once(':')
                    .ok_or_else(|| de::Error::custom("missing `:`"))?;

                match head {
                    "tmdb" => Ok(RemoteId::Tmdb {
                        id: tail.parse().map_err(E::custom)?,
                    }),
                    "tvdb" => Ok(RemoteId::Tvdb {
                        id: tail.parse().map_err(E::custom)?,
                    }),
                    "imdb" => Ok(RemoteId::Imdb {
                        id: Raw::new(tail)
                            .ok_or_else(|| de::Error::custom("overflowing imdb identifier"))?,
                    }),
                    kind => Err(de::Error::invalid_value(de::Unexpected::Str(kind), &self)),
                }
            }

            #[inline]
            fn visit_map<A>(self, mut map: A) -> std::result::Result<Self::Value, A::Error>
            where
                A: de::MapAccess<'de>,
            {
                let mut remote = None;
                let mut id = None;

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "remote" => {
                            remote = Some(map.next_value::<String>()?);
                        }
                        "id" => {
                            id = Some(map.next_value::<serde_json::Value>()?);
                        }
                        kind => {
                            return Err(de::Error::custom(format_args!("Unsupported key: {kind}")));
                        }
                    }
                }

                let (Some(remote), Some(id)) = (remote, id) else {
                    return Err(de::Error::custom("Missing remote or id"));
                };

                let id = id.into_deserializer();

                match remote.as_str() {
                    "tmdb" => Ok(RemoteId::Tmdb {
                        id: u32::deserialize(id).map_err(de::Error::custom)?,
                    }),
                    "tvdb" => Ok(RemoteId::Tvdb {
                        id: u32::deserialize(id).map_err(de::Error::custom)?,
                    }),
                    "imdb" => Ok(RemoteId::Imdb {
                        id: Raw::deserialize(id).map_err(de::Error::custom)?,
                    }),
                    kind => Err(de::Error::invalid_value(de::Unexpected::Str(kind), &self)),
                }
            }
        }

        deserializer.deserialize_any(RemoteIdVisitor)
    }
}

/// Graphics which have been customized.
#[derive(
    Debug, Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Hash, Serialize, Deserialize, Encode, Decode,
)]
#[serde(rename_all = "kebab-case")]
pub enum CustomGraphic {
    /// A custom series poster.
    Poster,
    /// A series banner.
    Banner,
}

/// Associated series graphics.
#[derive(Default, Debug, Clone, Serialize, Deserialize, Encode, Decode)]
#[serde(rename_all = "snake_case")]
pub struct MovieGraphics {
    /// Graphical elements which have been customized.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    #[musli(default, skip_encoding_if = BTreeSet::is_empty)]
    pub custom: BTreeSet<CustomGraphic>,
    /// Poster image.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[musli(default, skip_encoding_if = Option::is_none)]
    pub poster: Option<ImageV2>,
    /// Available alternative poster images.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    #[musli(default, skip_encoding_if = BTreeSet::is_empty)]
    pub posters: BTreeSet<ImageV2>,
    /// Banner image.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[musli(default, skip_encoding_if = Option::is_none)]
    pub banner: Option<ImageV2>,
    /// Available alternative banner images.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    #[musli(default, skip_encoding_if = BTreeSet::is_empty)]
    pub banners: BTreeSet<ImageV2>,
    /// Fanart image.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[musli(default, skip_encoding_if = Option::is_none)]
    pub fanart: Option<ImageV2>,
    /// Screencap for movie.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[musli(default, skip_encoding_if = Option::is_none)]
    pub screen_capture: Option<ImageV2>,
}

impl MovieGraphics {
    /// Merge one graphics from another.
    pub fn merge_from(&mut self, other: Self) {
        if !self.custom.contains(&CustomGraphic::Poster) {
            self.poster = other.poster;
        }

        if !self.custom.contains(&CustomGraphic::Banner) {
            self.banner = other.banner;
        }

        self.posters = other.posters;
        self.banners = other.banners;
        self.fanart = other.fanart;
    }

    fn is_empty(&self) -> bool {
        self.poster.is_none() && self.banner.is_none() && self.fanart.is_none()
    }
}

/// Associated series graphics.
#[derive(Default, Debug, Clone, Serialize, Deserialize, Encode, Decode)]
#[serde(rename_all = "snake_case")]
pub struct SeriesGraphics {
    /// Graphical elements which have been customized.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    #[musli(default, skip_encoding_if = BTreeSet::is_empty)]
    pub custom: BTreeSet<CustomGraphic>,
    /// Poster image.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[musli(default, skip_encoding_if = Option::is_none)]
    pub poster: Option<ImageV2>,
    /// Available alternative poster images.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    #[musli(default, skip_encoding_if = BTreeSet::is_empty)]
    pub posters: BTreeSet<ImageV2>,
    /// Banner image.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[musli(default, skip_encoding_if = Option::is_none)]
    pub banner: Option<ImageV2>,
    /// Available alternative banner images.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    #[musli(default, skip_encoding_if = BTreeSet::is_empty)]
    pub banners: BTreeSet<ImageV2>,
    /// Fanart image.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[musli(default, skip_encoding_if = Option::is_none)]
    pub fanart: Option<ImageV2>,
}

impl SeriesGraphics {
    /// Merge on graphics from another.
    pub fn merge_from(&mut self, other: Self) {
        if !self.custom.contains(&CustomGraphic::Poster) {
            self.poster = other.poster;
        }

        if !self.custom.contains(&CustomGraphic::Banner) {
            self.banner = other.banner;
        }

        self.posters = other.posters;
        self.banners = other.banners;
        self.fanart = other.fanart;
    }

    fn is_empty(&self) -> bool {
        self.poster.is_none() && self.banner.is_none() && self.fanart.is_none()
    }
}

/// A series.
#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
#[serde(rename_all = "snake_case")]
pub struct Series {
    /// Unique identifier for series.
    pub id: SeriesId,
    /// Title of the series.
    pub title: String,
    /// First air date of the series.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[musli(default, skip_encoding_if = Option::is_none, with = musli::serde)]
    pub first_air_date: Option<NaiveDate>,
    /// Overview of the series.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[musli(default, skip_encoding_if = String::is_empty)]
    pub overview: String,
    /// Series graphics.
    #[serde(default, skip_serializing_if = "SeriesGraphics::is_empty")]
    #[musli(default, skip_encoding_if = SeriesGraphics::is_empty)]
    pub graphics: SeriesGraphics,
    /// Indicates if the series is tracked or not, in that it will receive updates.
    #[serde(default)]
    #[musli(default)]
    pub tracked: bool,
    /// The remote identifier that is used to synchronize this series.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[musli(default, skip_encoding_if = Option::is_none)]
    pub remote_id: Option<RemoteId>,
}

impl Series {
    /// Get the poster of the series.
    pub fn poster(&self) -> Option<&ImageV2> {
        self.graphics.poster.as_ref()
    }

    /// Get the banner of the series.
    pub fn banner(&self) -> Option<&ImageV2> {
        self.graphics.banner.as_ref()
    }
}

/// Movie release kind.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Encode, Decode,
)]
#[serde(rename_all = "snake_case")]
pub enum MovieReleaseKind {
    Premiere,
    TheatricalLimited,
    Theatrical,
    Digital,
    Physical,
    Tv,
}

impl MovieReleaseKind {
    /// Test if release kind is digital or equivalent.
    pub fn is_digital(&self) -> bool {
        matches!(
            self,
            MovieReleaseKind::Digital | MovieReleaseKind::Physical | MovieReleaseKind::Tv
        )
    }
}

impl fmt::Display for MovieReleaseKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MovieReleaseKind::Premiere => write!(f, "premiere"),
            MovieReleaseKind::TheatricalLimited => write!(f, "theatrical (limited)"),
            MovieReleaseKind::Theatrical => write!(f, "theatrical"),
            MovieReleaseKind::Digital => write!(f, "digital"),
            MovieReleaseKind::Physical => write!(f, "physical"),
            MovieReleaseKind::Tv => write!(f, "tv"),
        }
    }
}

/// A movie release date.
#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
#[serde(rename_all = "snake_case")]
pub struct MovieReleaseDate {
    #[musli(with = musli::serde)]
    pub date: DateTime<Utc>,
    pub kind: MovieReleaseKind,
}

/// Release dates for a given country.
#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
#[serde(rename_all = "snake_case")]
pub struct MovieReleaseDates {
    pub country: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[musli(default, skip_encoding_if = Vec::is_empty)]
    pub dates: Vec<MovieReleaseDate>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
#[serde(rename_all = "snake_case")]
pub struct MovieEarliestReleaseDate {
    pub country: String,
    pub kind: MovieReleaseKind,
    #[musli(with = musli::serde)]
    pub date: DateTime<Utc>,
}

/// A movie.
#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
#[serde(rename_all = "snake_case")]
pub struct Movie {
    /// Unique identifier for movie.
    pub id: MovieId,
    /// The title of the movie.
    pub title: String,
    /// First screen date of the movie.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[musli(default, skip_encoding_if = Option::is_none, with = musli::serde)]
    pub release_date: Option<NaiveDate>,
    /// The overview of a movie.
    pub overview: String,
    /// Movie graphics.
    #[serde(default, skip_serializing_if = "MovieGraphics::is_empty")]
    #[musli(default, skip_encoding_if = MovieGraphics::is_empty)]
    pub graphics: MovieGraphics,
    /// The remote identifier that is used to synchronize this movie.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[musli(default, skip_encoding_if = Option::is_none)]
    pub remote_id: Option<RemoteId>,
    /// Release dates.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[musli(default, skip_encoding_if = Vec::is_empty)]
    pub release_dates: Vec<MovieReleaseDates>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[musli(default, skip_encoding_if = Vec::is_empty)]
    pub earliest_releases: Vec<MovieEarliestReleaseDate>,
}

impl Movie {
    /// Get the earliest relase date.
    pub fn earliest(&self) -> Option<DateTime<Utc>> {
        self.earliest_release_date().or(self.release())
    }

    /// Get earliest actual release date.
    pub fn earliest_release_date(&self) -> Option<DateTime<Utc>> {
        self.earliest_by_kind()
            .iter()
            .filter(|e| e.kind.is_digital())
            .map(|e| e.date)
            .min()
    }

    /// Get a batch of earliest release dates.
    pub fn earliest_by_kind(&self) -> &[MovieEarliestReleaseDate] {
        &self.earliest_releases
    }

    /// Get the poster of the movie.
    pub fn poster(&self) -> Option<&ImageV2> {
        self.graphics.poster.as_ref()
    }

    /// Get the banner of the movie.
    pub fn banner(&self) -> Option<&ImageV2> {
        self.graphics.banner.as_ref()
    }

    /// Test if episode will release in the future.
    pub fn will_release(&self, today: &NaiveDate) -> bool {
        let Some(release_date) = self.earliest_release_date() else {
            return false;
        };

        release_date.date_naive() > *today
    }

    /// Test if the given episode will be released.
    pub fn has_released(&self, today: &NaiveDate) -> bool {
        let Some(release_date) = self.earliest_release_date() else {
            return false;
        };

        release_date.date_naive() <= *today
    }

    /// Get release timestamp.
    pub fn release(&self) -> Option<DateTime<Utc>> {
        self.release_date.as_ref().and_then(|&d| {
            Some(DateTime::from_naive_utc_and_offset(
                d.and_hms_opt(0, 0, 0)?,
                Utc,
            ))
        })
    }
}

/// A season in a series.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", untagged)]
pub enum WatchedKind {
    /// The watch kind is a series.
    Series {
        /// Identifier of watched series.
        series: SeriesId,
        /// Identifier of watched episode.
        episode: EpisodeId,
    },
    /// The watched movie.
    Movie { movie: MovieId },
}

/// A season in a series.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Watched {
    /// Unique identifier for this watch.
    pub id: WatchedId,
    /// Timestamp when it was watched.
    pub timestamp: DateTime<Utc>,
    /// Watched kind.
    #[serde(flatten)]
    pub kind: WatchedKind,
}

/// The kind of a pending item.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PendingKind {
    Episode {
        series: SeriesId,
        episode: EpisodeId,
    },
    Movie {
        movie: MovieId,
    },
}

/// A pending thing to watch.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Pending {
    pub timestamp: DateTime<Utc>,
    #[serde(flatten)]
    pub kind: PendingKind,
}

impl Pending {
    /// Access the raw id for the pending item.
    pub fn id(&self) -> &Uuid {
        match &self.kind {
            PendingKind::Episode { series, .. } => series.id(),
            PendingKind::Movie { movie } => movie.id(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SearchSeries {
    pub id: RemoteId,
    pub name: String,
    pub poster: Option<ImageV2>,
    pub overview: String,
    pub first_aired: Option<NaiveDate>,
}

impl SearchSeries {
    pub fn poster(&self) -> Option<&ImageV2> {
        self.poster.as_ref()
    }
}

#[derive(Debug, Clone)]
pub struct SearchMovie {
    pub id: RemoteId,
    pub title: String,
    pub poster: Option<ImageV2>,
    pub overview: String,
    pub release_date: Option<NaiveDate>,
}

impl SearchMovie {
    /// Get poster for searched movie.
    pub fn poster(&self) -> Option<&ImageV2> {
        self.poster.as_ref()
    }
}

/// Generate a 16-byte hash.
fn hash128<T>(value: &T) -> u128
where
    T: std::hash::Hash,
{
    use twox_hash::xxh3::HasherExt;
    let mut hasher = twox_hash::Xxh3Hash128::default();
    std::hash::Hash::hash(value, &mut hasher);
    hasher.finish_ext()
}
