mod etag;
mod hex;
mod raw;

use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use anyhow::Result;
use chrono::{DateTime, NaiveDate, Utc};
use relative_path::{RelativePath, RelativePathBuf};
use serde::de::IntoDeserializer;
use serde::{de, ser, Deserialize, Serialize};
use uuid::Uuid;

pub(crate) use self::etag::Etag;
pub(crate) use self::hex::Hex;
pub(crate) use self::raw::Raw;

macro_rules! id {
    ($name:ident) => {
        #[derive(
            Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        #[repr(transparent)]
        #[serde(transparent)]
        pub(crate) struct $name(Uuid);

        impl $name {
            /// Generate a new random series identifier.
            #[inline]
            #[allow(unused)]
            pub(crate) fn random() -> Self {
                Self(Uuid::new_v4())
            }
        }

        impl fmt::Display for $name {
            #[inline]
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }

        impl FromStr for $name {
            type Err = uuid::Error;

            #[inline]
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Ok(Self(uuid::Uuid::from_str(s)?))
            }
        }
    };
}

id!(SeriesId);
id!(EpisodeId);
id!(MovieId);

impl SeriesId {
    /// Get underlying uuid.
    #[inline]
    pub(crate) fn id(&self) -> &Uuid {
        &self.0
    }
}

#[derive(Default, Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ThemeType {
    Light,
    #[default]
    Dark,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum SearchKind {
    Tvdb,
    #[default]
    Tmdb,
}

impl fmt::Display for SearchKind {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SearchKind::Tvdb => write!(f, "thetvdb.com"),
            SearchKind::Tmdb => write!(f, "themoviedb.com"),
        }
    }
}

#[inline]
fn default_days() -> u64 {
    7
}

#[inline]
fn default_dashboard_limit() -> usize {
    1
}

#[inline]
fn default_dashboard_page() -> usize {
    6
}

#[inline]
fn default_schedule_limit() -> usize {
    1
}

#[inline]
fn default_schedule_page() -> usize {
    7
}

/// The state for the settings page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Config {
    #[serde(default)]
    pub(crate) theme: ThemeType,
    #[serde(default)]
    pub(crate) tvdb_legacy_apikey: String,
    #[serde(default)]
    pub(crate) tmdb_api_key: String,
    #[serde(default = "default_days")]
    pub(crate) schedule_duration_days: u64,
    #[serde(default)]
    pub(crate) search_kind: SearchKind,
    #[serde(default = "default_dashboard_limit")]
    pub(crate) dashboard_limit: usize,
    #[serde(default = "default_dashboard_page")]
    pub(crate) dashboard_page: usize,
    #[serde(default = "default_schedule_limit")]
    pub(crate) schedule_limit: usize,
    #[serde(default = "default_schedule_page")]
    pub(crate) schedule_page: usize,
}

impl Config {
    pub(crate) fn dashboard_limit(&self) -> usize {
        self.dashboard_limit.max(1) * self.dashboard_page.max(1)
    }

    pub(crate) fn dashboard_page(&self) -> usize {
        self.dashboard_page.max(1)
    }

    pub(crate) fn schedule_page(&self) -> usize {
        self.schedule_page.max(1)
    }
}

impl Default for Config {
    #[inline]
    fn default() -> Self {
        Self {
            theme: Default::default(),
            tvdb_legacy_apikey: Default::default(),
            tmdb_api_key: Default::default(),
            schedule_duration_days: default_days(),
            search_kind: SearchKind::default(),
            dashboard_limit: default_dashboard_limit(),
            dashboard_page: default_dashboard_page(),
            schedule_limit: default_schedule_limit(),
            schedule_page: default_schedule_page(),
        }
    }
}

impl Config {
    /// Build iced theme.
    #[inline]
    pub(crate) fn theme(&self) -> iced::Theme {
        match self.theme {
            ThemeType::Light => iced::Theme::Light,
            ThemeType::Dark => iced::Theme::Dark,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "type")]
pub(crate) enum RemoteId {
    Series {
        uuid: SeriesId,
        remotes: Vec<RemoteSeriesId>,
    },
    Episode {
        uuid: EpisodeId,
        remotes: Vec<RemoteEpisodeId>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum RemoteSeriesId {
    Tvdb { id: u32 },
    Tmdb { id: u32 },
    Imdb { id: Raw<16> },
}

impl RemoteSeriesId {
    pub(crate) fn url(&self) -> String {
        match self {
            RemoteSeriesId::Tvdb { id } => {
                format!("https://thetvdb.com/search?query={id}")
            }
            RemoteSeriesId::Tmdb { id } => {
                format!("https://www.themoviedb.org/tv/{id}")
            }
            RemoteSeriesId::Imdb { id } => {
                format!("https://www.imdb.com/title/{id}/")
            }
        }
    }
}

impl fmt::Display for RemoteSeriesId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RemoteSeriesId::Tvdb { id } => {
                write!(f, "tvdb:{id}")
            }
            RemoteSeriesId::Tmdb { id } => {
                write!(f, "tmdb:{id}")
            }
            RemoteSeriesId::Imdb { id } => {
                write!(f, "imdb:{id}")
            }
        }
    }
}

impl Serialize for RemoteSeriesId {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for RemoteSeriesId {
    #[inline]
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct RemoteSeriesIdVisitor;

        impl<'de> de::Visitor<'de> for RemoteSeriesIdVisitor {
            type Value = RemoteSeriesId;

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
                    "tmdb" => Ok(RemoteSeriesId::Tmdb {
                        id: tail.parse().map_err(E::custom)?,
                    }),
                    "tvdb" => Ok(RemoteSeriesId::Tvdb {
                        id: tail.parse().map_err(E::custom)?,
                    }),
                    "imdb" => Ok(RemoteSeriesId::Imdb {
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
                    "tmdb" => Ok(RemoteSeriesId::Tmdb {
                        id: u32::deserialize(id).map_err(de::Error::custom)?,
                    }),
                    "tvdb" => Ok(RemoteSeriesId::Tvdb {
                        id: u32::deserialize(id).map_err(de::Error::custom)?,
                    }),
                    "imdb" => Ok(RemoteSeriesId::Imdb {
                        id: Raw::deserialize(id).map_err(de::Error::custom)?,
                    }),
                    kind => Err(de::Error::invalid_value(de::Unexpected::Str(kind), &self)),
                }
            }
        }

        deserializer.deserialize_any(RemoteSeriesIdVisitor)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum RemoteEpisodeId {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "remote")]
pub(crate) enum RemoteMovieId {
    Tmdb { id: u32 },
    Imdb { id: Raw<16> },
}

impl RemoteMovieId {
    pub(crate) fn url(&self) -> String {
        match self {
            RemoteMovieId::Tmdb { id } => {
                format!("https://www.themoviedb.org/tv/{id}")
            }
            RemoteMovieId::Imdb { id } => {
                format!("https://www.imdb.com/title/{id}/")
            }
        }
    }
}

impl fmt::Display for RemoteMovieId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RemoteMovieId::Tmdb { id } => {
                write!(f, "themoviedb.org ({id})")
            }
            RemoteMovieId::Imdb { id } => {
                write!(f, "imdb.com ({id})")
            }
        }
    }
}

/// Associated series graphics.
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct SeriesGraphics {
    /// Poster image.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) poster: Option<ImageV2>,
    /// Banner image.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) banner: Option<ImageV2>,
    /// Fanart image.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) fanart: Option<ImageV2>,
}

impl SeriesGraphics {
    fn is_empty(&self) -> bool {
        self.poster.is_none() && self.banner.is_none() && self.fanart.is_none()
    }
}

/// A series.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct Series {
    /// Unique identifier for series.
    pub(crate) id: SeriesId,
    /// Title of the series.
    pub(crate) title: String,
    /// First air date of the series.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) first_air_date: Option<NaiveDate>,
    /// Overview of the series.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub(crate) overview: String,
    /// Poster image.
    #[serde(default, rename = "poster", skip_serializing)]
    pub(crate) compat_poster: Option<Image>,
    /// Banner image.
    #[serde(default, rename = "banner", skip_serializing)]
    pub(crate) compat_banner: Option<Image>,
    /// Fanart image.
    #[serde(default, rename = "fanart", skip_serializing)]
    pub(crate) compat_fanart: Option<Image>,
    /// Series graphics.
    #[serde(default, skip_serializing_if = "SeriesGraphics::is_empty")]
    pub(crate) graphics: SeriesGraphics,
    /// Indicates if the series is tracked or not, in that it will receive updates.
    #[serde(default)]
    pub(crate) tracked: bool,
    /// Locally known last modified timestamp.
    #[serde(rename = "last_modified", default, skip_serializing)]
    pub(crate) compat_last_modified: Option<DateTime<Utc>>,
    /// Locally known last etag.
    #[serde(rename = "last_etag", default, skip_serializing)]
    pub(crate) compat_last_etag: Option<Etag>,
    /// Last sync time for each remote.
    #[serde(rename = "last_sync", default, skip_serializing, with = "btree_as_vec")]
    pub(crate) compat_last_sync: BTreeMap<RemoteSeriesId, DateTime<Utc>>,
    /// The remote identifier that is used to synchronize this series.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) remote_id: Option<RemoteSeriesId>,
}

impl Series {
    /// Get the poster of the series.
    pub(crate) fn poster(&self) -> Option<&ImageV2> {
        self.graphics.poster.as_ref()
    }

    /// Get the banner of the series.
    pub(crate) fn banner(&self) -> Option<&ImageV2> {
        self.graphics.banner.as_ref()
    }
}

/// A movie.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct Movie {
    /// Unique identifier for movie.
    pub(crate) id: MovieId,
    /// The title of the movie.
    pub(crate) title: String,
    /// The remote identifier that is used to synchronize this movie.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) remote_id: Option<RemoteMovieId>,
}

pub(crate) mod btree_as_vec {
    use std::collections::BTreeMap;
    use std::fmt;

    use serde::de;
    use serde::ser;
    use serde::ser::SerializeSeq;

    #[allow(unused)]
    pub(crate) fn serialize<S, K, V>(
        value: &BTreeMap<K, V>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
        K: ser::Serialize,
        V: ser::Serialize,
    {
        let mut serializer = serializer.serialize_seq(Some(value.len()))?;

        for (key, value) in value {
            serializer.serialize_element(&(key, value))?;
        }

        serializer.end()
    }

    pub(crate) fn deserialize<'de, S, K, V>(deserializer: S) -> Result<BTreeMap<K, V>, S::Error>
    where
        S: de::Deserializer<'de>,
        K: Ord + de::Deserialize<'de>,
        V: de::Deserialize<'de>,
    {
        return deserializer.deserialize_seq(Visitor(BTreeMap::new()));
    }

    impl<'de, K, V> de::Visitor<'de> for Visitor<K, V>
    where
        K: Ord + de::Deserialize<'de>,
        V: de::Deserialize<'de>,
    {
        type Value = BTreeMap<K, V>;

        #[inline]
        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "a sequence")
        }

        #[inline]
        fn visit_seq<A>(mut self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: de::SeqAccess<'de>,
        {
            while let Some(element) = seq.next_element::<(K, V)>()? {
                self.0.insert(element.0, element.1);
            }

            Ok(self.0)
        }
    }

    struct Visitor<K, V>(BTreeMap<K, V>);
}

/// A season in a series.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", untagged)]
pub(crate) enum WatchedKind {
    /// The watch kind is a series.
    Series {
        /// Identifier of watched series.
        series: SeriesId,
        /// Identifier of watched episode.
        episode: EpisodeId,
    },
}

/// A season in a series.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct Watched {
    /// Unique identifier for this watch.
    pub(crate) id: Uuid,
    /// Timestamp when it was watched.
    pub(crate) timestamp: DateTime<Utc>,
    /// Watched kind.
    #[serde(flatten)]
    pub(crate) kind: WatchedKind,
}

/// Season number.
#[derive(
    Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(untagged)]
pub(crate) enum SeasonNumber {
    /// Season used for non-numbered episodes.
    #[default]
    Specials,
    /// A regular numbered season.
    Number(u32),
}

impl SeasonNumber {
    #[inline]
    pub(crate) fn is_special(&self) -> bool {
        matches!(self, SeasonNumber::Specials)
    }

    /// Build season title.
    pub(crate) fn short(&self) -> SeasonShort<'_> {
        SeasonShort { season: self }
    }
}

pub(crate) struct SeasonShort<'a> {
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
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct SeasonGraphics {
    /// Poster for season.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) poster: Option<ImageV2>,
}

impl SeasonGraphics {
    fn is_empty(&self) -> bool {
        self.poster.is_none()
    }
}

/// A season in a series.
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct Season {
    /// The number of the season.
    #[serde(default, skip_serializing_if = "SeasonNumber::is_special")]
    pub(crate) number: SeasonNumber,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) air_date: Option<NaiveDate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) name: Option<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub(crate) overview: String,
    #[serde(default, rename = "poster", skip_serializing_if = "Option::is_none")]
    pub(crate) compat_poster: Option<Image>,
    #[serde(default, skip_serializing_if = "SeasonGraphics::is_empty")]
    pub(crate) graphics: SeasonGraphics,
}

impl Season {
    /// Get the poster of the season.
    pub(crate) fn poster(&self) -> Option<&ImageV2> {
        self.graphics.poster.as_ref()
    }
}

/// Associated episode graphics.
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct EpisodeGraphics {
    /// Filename for episode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) filename: Option<ImageV2>,
}

impl EpisodeGraphics {
    fn is_empty(&self) -> bool {
        self.filename.is_none()
    }
}

/// An episode in a series.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct Episode {
    /// Uuid of the watched episode.
    pub(crate) id: EpisodeId,
    /// Name of the episode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) name: Option<String>,
    /// Overview of the episode.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub(crate) overview: String,
    /// Absolute number in the series.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) absolute_number: Option<u32>,
    /// Season number.
    #[serde(default, skip_serializing_if = "SeasonNumber::is_special")]
    pub(crate) season: SeasonNumber,
    /// Episode number inside of its season.
    pub(crate) number: u32,
    /// Air date of the episode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) aired: Option<NaiveDate>,
    /// Episode image.
    #[serde(default, rename = "filename", skip_serializing_if = "Option::is_none")]
    pub(crate) compat_filename: Option<Image>,
    /// Episode graphics.
    #[serde(default, skip_serializing_if = "EpisodeGraphics::is_empty")]
    pub(crate) graphics: EpisodeGraphics,
    /// The remote identifier that is used to synchronize this episode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) remote_id: Option<RemoteEpisodeId>,
}

impl Episode {
    /// Get filename for episode.
    pub(crate) fn filename(&self) -> Option<&ImageV2> {
        self.graphics.filename.as_ref()
    }

    /// Test if the given episode has aired by the provided timestamp.
    pub(crate) fn has_aired(&self, today: &NaiveDate) -> bool {
        let Some(aired) = &self.aired else {
            return false;
        };

        *aired <= *today
    }

    /// Get aired timestamp.
    pub(crate) fn aired_timestamp(&self) -> Option<DateTime<Utc>> {
        self.aired
            .as_ref()
            .and_then(|&d| Some(DateTime::from_utc(d.and_hms_opt(12, 0, 0)?, Utc)))
    }
}

impl fmt::Display for Episode {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} / {}", self.season, self.number)
    }
}

/// Image format in use.
#[derive(
    Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize,
)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ImageExt {
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
    Legacy(u64, ArtKind, Hex<16>),
    V4(u64, ArtKind, Hex<16>),
    Banner(Hex<16>),
    BannerSuffixed(u64, Raw<16>),
    Graphical(Hex<16>),
    GraphicalSuffixed(u64, Raw<16>),
    Fanart(Hex<16>),
    FanartSuffixed(u64, Raw<16>),
    ScreenCap(u64, Hex<16>),
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

/// The hash of an image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub(crate) struct ImageHash(u128);

impl ImageHash {
    pub(crate) fn as_u128(&self) -> u128 {
        self.0
    }
}

/// The identifier of an image.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum ImageV2 {
    /// An image from thetvdb.com
    Tvdb { uri: Box<RelativePath> },
    /// An image from themoviedb.org
    Tmdb { uri: Box<RelativePath> },
}

impl ImageV2 {
    /// Generate an image hash.
    pub(crate) fn hash(&self) -> ImageHash {
        ImageHash(match self {
            ImageV2::Tvdb { uri } => crate::cache::hash128(&(0xd410b8f4u32, uri)),
            ImageV2::Tmdb { uri } => crate::cache::hash128(&(0xc66bff3eu32, uri)),
        })
    }

    /// Construct a new tvbd image.
    pub(crate) fn tvdb<S>(string: &S) -> Option<Self>
    where
        S: ?Sized + AsRef<str>,
    {
        Some(string.as_ref().trim_start_matches('/'))
            .filter(|s| !s.is_empty())
            .map(|uri| Self::Tvdb { uri: uri.into() })
    }

    /// Construct a new tmdb image.
    pub(crate) fn tmdb<S>(string: &S) -> Option<Self>
    where
        S: ?Sized + AsRef<str>,
    {
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
                let (head, uri) = v
                    .split_once(':')
                    .ok_or_else(|| de::Error::custom("missing `:`"))?;

                match head {
                    "tmdb" => Ok(ImageV2::Tmdb { uri: uri.into() }),
                    "tvdb" => Ok(ImageV2::Tvdb { uri: uri.into() }),
                    kind => Err(de::Error::invalid_value(de::Unexpected::Str(kind), &self)),
                }
            }
        }

        deserializer.deserialize_str(ImageV2Visitor)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum TaskId {
    /// Check for updates.
    CheckForUpdates {
        series_id: SeriesId,
        remote_id: RemoteSeriesId,
    },
    /// Task to download series data.
    DownloadSeriesById { series_id: SeriesId },
    /// Task to add a series by a remote identifier.
    DownloadSeriesByRemoteId { remote_id: RemoteSeriesId },
    /// Task to add download a movie by a remote identifier.
    DownloadMovieByRemoteId { remote_id: RemoteMovieId },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum TaskKind {
    /// Check for updates.
    CheckForUpdates {
        series_id: SeriesId,
        remote_id: RemoteSeriesId,
    },
    /// Task to download series data.
    DownloadSeriesById {
        series_id: SeriesId,
        remote_id: RemoteSeriesId,
        last_modified: Option<DateTime<Utc>>,
    },
    /// Task to add a series by a remote identifier.
    DownloadSeriesByRemoteId { remote_id: RemoteSeriesId },
    /// Task to add download a movie by a remote identifier.
    #[allow(unused)]
    DownloadMovieByRemoteId { remote_id: RemoteMovieId },
}

impl TaskKind {
    pub(crate) fn id(&self) -> TaskId {
        match *self {
            TaskKind::CheckForUpdates {
                series_id,
                remote_id,
                ..
            } => TaskId::CheckForUpdates {
                series_id,
                remote_id,
            },
            TaskKind::DownloadSeriesById { series_id, .. } => {
                TaskId::DownloadSeriesById { series_id }
            }
            TaskKind::DownloadSeriesByRemoteId { remote_id, .. } => {
                TaskId::DownloadSeriesByRemoteId { remote_id }
            }
            TaskKind::DownloadMovieByRemoteId { remote_id } => {
                TaskId::DownloadMovieByRemoteId { remote_id }
            }
        }
    }
}

/// A task in a queue.
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use]
pub(crate) struct Task {
    /// The identifier of the task.
    pub(crate) id: Uuid,
    /// The kind of the task.
    pub(crate) kind: TaskKind,
    /// When the task is scheduled for.
    pub(crate) scheduled: Option<DateTime<Utc>>,
}

impl Task {
    /// Test if task involves the given series.
    pub(crate) fn is_series(&self, id: &SeriesId) -> bool {
        match &self.kind {
            TaskKind::DownloadSeriesById { series_id, .. } => *series_id == *id,
            TaskKind::CheckForUpdates { series_id, .. } => *series_id == *id,
            TaskKind::DownloadSeriesByRemoteId { .. } => false,
            TaskKind::DownloadMovieByRemoteId { .. } => false,
        }
    }
}

/// A pending thing to watch.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub(crate) struct Pending {
    pub(crate) series: SeriesId,
    pub(crate) episode: EpisodeId,
    pub(crate) timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub(crate) struct SearchSeries {
    pub(crate) id: RemoteSeriesId,
    pub(crate) name: String,
    pub(crate) poster: Option<ImageV2>,
    pub(crate) overview: String,
    pub(crate) first_aired: Option<NaiveDate>,
}

impl SearchSeries {
    pub(crate) fn poster(&self) -> Option<&ImageV2> {
        self.poster.as_ref()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SearchMovie {
    pub(crate) id: RemoteMovieId,
    pub(crate) title: String,
    pub(crate) poster: Option<ImageV2>,
    pub(crate) overview: String,
    pub(crate) release_date: Option<NaiveDate>,
}

impl SearchMovie {
    /// Get poster for searched movie.
    pub(crate) fn poster(&self) -> Option<&ImageV2> {
        self.poster.as_ref()
    }
}

/// A series that is scheduled to be aired.
pub(crate) struct ScheduledSeries {
    pub(crate) series_id: SeriesId,
    pub(crate) episodes: Vec<EpisodeId>,
}

/// A scheduled day.
pub(crate) struct ScheduledDay {
    pub(crate) date: NaiveDate,
    pub(crate) schedule: Vec<ScheduledSeries>,
}
