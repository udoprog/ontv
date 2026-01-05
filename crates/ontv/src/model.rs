mod etag;
mod raw;

use core::cmp::Ordering;
use std::collections::btree_map;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::str::FromStr;

use anyhow::Result;
use api::{ImageV2, SeasonNumber};
use chrono::{DateTime, NaiveDate, Utc};
use relative_path::RelativePath;
use serde::de::IntoDeserializer;
use serde::{de, ser, Deserialize, Serialize};
use uuid::Uuid;

pub(crate) use self::etag::Etag;
pub(crate) use self::raw::Raw;
pub(crate) use api::config::{Config, ThemeType};

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

            /// Access underlying id.
            #[inline]
            #[allow(unused)]
            pub(crate) fn id(&self) -> &Uuid {
                &self.0
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
id!(WatchedId);
id!(TaskId);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "type")]
pub(crate) enum RemoteIds {
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
pub(crate) enum RemoteSeasonId {
    Tmdb { id: u32, season: SeasonNumber },
    Imdb { id: Raw<16>, season: SeasonNumber },
}

impl RemoteSeasonId {
    pub(crate) fn url(&self) -> String {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum RemoteId {
    Tvdb { id: u32 },
    Tmdb { id: u32 },
    Imdb { id: Raw<16> },
}

impl RemoteId {
    /// Coerce into a remote season.
    pub(crate) fn into_season(self, season: SeasonNumber) -> Option<RemoteSeasonId> {
        match self {
            RemoteId::Tmdb { id } => Some(RemoteSeasonId::Tmdb { id, season }),
            RemoteId::Imdb { id } => Some(RemoteSeasonId::Imdb { id, season }),
            _ => None,
        }
    }

    pub(crate) fn url(&self) -> String {
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
    pub(crate) fn is_supported(&self) -> bool {
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

/// Graphics which have been customized.
#[derive(Debug, Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum CustomGraphic {
    /// A custom series poster.
    Poster,
    /// A series banner.
    Banner,
}

/// Associated series graphics.
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct MovieGraphics {
    /// Graphical elements which have been customized.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub(crate) custom: BTreeSet<CustomGraphic>,
    /// Poster image.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) poster: Option<ImageV2>,
    /// Available alternative poster images.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub(crate) posters: BTreeSet<ImageV2>,
    /// Banner image.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) banner: Option<ImageV2>,
    /// Available alternative banner images.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub(crate) banners: BTreeSet<ImageV2>,
    /// Fanart image.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) fanart: Option<ImageV2>,
    /// Screencap for movie.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) screen_capture: Option<ImageV2>,
}

impl MovieGraphics {
    fn merge_from(&mut self, other: Self) {
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
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct SeriesGraphics {
    /// Graphical elements which have been customized.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub(crate) custom: BTreeSet<CustomGraphic>,
    /// Poster image.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) poster: Option<ImageV2>,
    /// Available alternative poster images.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub(crate) posters: BTreeSet<ImageV2>,
    /// Banner image.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) banner: Option<ImageV2>,
    /// Available alternative banner images.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub(crate) banners: BTreeSet<ImageV2>,
    /// Fanart image.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) fanart: Option<ImageV2>,
}

impl SeriesGraphics {
    fn merge_from(&mut self, other: Self) {
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
    /// Series graphics.
    #[serde(default, skip_serializing_if = "SeriesGraphics::is_empty")]
    pub(crate) graphics: SeriesGraphics,
    /// Indicates if the series is tracked or not, in that it will receive updates.
    #[serde(default)]
    pub(crate) tracked: bool,
    /// The remote identifier that is used to synchronize this series.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) remote_id: Option<RemoteId>,
}

impl Series {
    /// Construct a new series from a series update.
    #[allow(deprecated)]
    pub(crate) fn new_series(update: crate::backend::UpdateSeries) -> Self {
        Self {
            id: update.id,
            title: update.title,
            first_air_date: update.first_air_date,
            overview: update.overview,
            graphics: update.graphics,
            remote_id: Some(update.remote_id),
            tracked: true,
        }
    }

    /// Merge this series from another.
    pub(crate) fn merge_from(&mut self, other: crate::backend::UpdateSeries) {
        self.title = other.title;
        self.first_air_date = other.first_air_date;
        self.overview = other.overview;
        self.graphics.merge_from(other.graphics);
        self.remote_id = Some(other.remote_id);
    }

    /// Get the poster of the series.
    pub(crate) fn poster(&self) -> Option<&ImageV2> {
        self.graphics.poster.as_ref()
    }

    /// Get the banner of the series.
    pub(crate) fn banner(&self) -> Option<&ImageV2> {
        self.graphics.banner.as_ref()
    }
}

/// Movie release kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MovieReleaseKind {
    Premiere,
    TheatricalLimited,
    Theatrical,
    Digital,
    Physical,
    Tv,
}

impl MovieReleaseKind {
    /// Test if release kind is digital or equivalent.
    pub(crate) fn is_digital(&self) -> bool {
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
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct MovieReleaseDate {
    pub(crate) date: DateTime<Utc>,
    pub(crate) kind: MovieReleaseKind,
}

/// Release dates for a given country.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct MovieReleaseDates {
    pub(crate) country: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) dates: Vec<MovieReleaseDate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct MovieEarliestReleaseDate {
    pub(crate) country: String,
    pub(crate) kind: MovieReleaseKind,
    pub(crate) date: DateTime<Utc>,
}

/// A movie.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct Movie {
    /// Unique identifier for movie.
    pub(crate) id: MovieId,
    /// The title of the movie.
    pub(crate) title: String,
    /// First screen date of the movie.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) release_date: Option<NaiveDate>,
    /// The overview of a movie.
    pub(crate) overview: String,
    /// Movie graphics.
    #[serde(default, skip_serializing_if = "MovieGraphics::is_empty")]
    pub(crate) graphics: MovieGraphics,
    /// The remote identifier that is used to synchronize this movie.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) remote_id: Option<RemoteId>,
    /// Release dates.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) release_dates: Vec<MovieReleaseDates>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) earliest_releases: Vec<MovieEarliestReleaseDate>,
}

impl Movie {
    /// Construct a new series from a series update.
    pub(crate) fn new_movie(update: crate::backend::UpdateMovie) -> Self {
        let earliest_releases = build_earliest_releases(&update.release_dates);

        Self {
            id: update.id,
            title: update.title,
            release_date: update.release_date,
            overview: update.overview,
            graphics: update.graphics,
            remote_id: Some(update.remote_id),
            release_dates: update.release_dates,
            earliest_releases,
        }
    }

    /// Get the earliest relase date.
    pub(crate) fn earliest(&self) -> Option<DateTime<Utc>> {
        self.earliest_release_date().or(self.release())
    }

    /// Get earliest actual release date.
    pub(crate) fn earliest_release_date(&self) -> Option<DateTime<Utc>> {
        self.earliest_by_kind()
            .iter()
            .filter(|e| e.kind.is_digital())
            .map(|e| e.date)
            .min()
    }

    /// Get a batch of earliest release dates.
    pub(crate) fn earliest_by_kind(&self) -> &[MovieEarliestReleaseDate] {
        &self.earliest_releases
    }

    /// Merge this movie from an update.
    pub(crate) fn merge_from(&mut self, other: crate::backend::UpdateMovie) {
        let earliest_releases = build_earliest_releases(&other.release_dates);

        self.title = other.title;
        self.release_date = other.release_date;
        self.overview = other.overview;
        self.graphics.merge_from(other.graphics);
        self.remote_id = Some(other.remote_id);
        self.release_dates = other.release_dates;
        self.earliest_releases = earliest_releases;
    }

    /// Get the poster of the movie.
    pub(crate) fn poster(&self) -> Option<&ImageV2> {
        self.graphics.poster.as_ref()
    }

    /// Get the banner of the movie.
    pub(crate) fn banner(&self) -> Option<&ImageV2> {
        self.graphics.banner.as_ref()
    }

    /// Test if episode will release in the future.
    pub(crate) fn will_release(&self, today: &NaiveDate) -> bool {
        let Some(release_date) = self.earliest_release_date() else {
            return false;
        };

        release_date.date_naive() > *today
    }

    /// Test if the given episode will be released.
    pub(crate) fn has_released(&self, today: &NaiveDate) -> bool {
        let Some(release_date) = self.earliest_release_date() else {
            return false;
        };

        release_date.date_naive() <= *today
    }

    /// Get release timestamp.
    pub(crate) fn release(&self) -> Option<DateTime<Utc>> {
        self.release_date.as_ref().and_then(|&d| {
            Some(DateTime::from_naive_utc_and_offset(
                d.and_hms_opt(0, 0, 0)?,
                Utc,
            ))
        })
    }
}

fn build_earliest_releases(release_dates: &[MovieReleaseDates]) -> Vec<MovieEarliestReleaseDate> {
    fn country_to_prio(country: &str) -> u32 {
        match country {
            "US" => 10,
            "GB" => 9,
            _ => 0,
        }
    }

    fn less_important(a: &str, b: &str) -> bool {
        country_to_prio(a) < country_to_prio(b)
    }

    let mut by_kind = BTreeMap::<MovieReleaseKind, MovieEarliestReleaseDate>::new();

    for country in release_dates {
        for date in &country.dates {
            match by_kind.entry(date.kind) {
                btree_map::Entry::Occupied(e) => {
                    let e = e.into_mut();

                    if let ordering @ (Ordering::Less | Ordering::Equal) = e.date.cmp(&date.date) {
                        if matches!(ordering, Ordering::Equal if less_important(&e.country, &country.country))
                        {
                            *e = MovieEarliestReleaseDate {
                                country: country.country.clone(),
                                date: date.date,
                                kind: date.kind,
                            };
                        }
                    }
                }
                btree_map::Entry::Vacant(e) => {
                    e.insert(MovieEarliestReleaseDate {
                        country: country.country.clone(),
                        date: date.date,
                        kind: date.kind,
                    });
                }
            }
        }
    }

    by_kind.into_values().collect()
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
    /// The watched movie.
    Movie { movie: MovieId },
}

/// A season in a series.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct Watched {
    /// Unique identifier for this watch.
    pub(crate) id: WatchedId,
    /// Timestamp when it was watched.
    pub(crate) timestamp: DateTime<Utc>,
    /// Watched kind.
    #[serde(flatten)]
    pub(crate) kind: WatchedKind,
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

    /// Test if episode will air in the future.
    ///
    /// This ignores episodes without an air date.
    pub(crate) fn will_air(&self, today: &NaiveDate) -> bool {
        let Some(aired) = self.aired else {
            return false;
        };

        aired > *today
    }

    /// Test if the given episode has aired by the provided timestamp.
    pub(crate) fn has_aired(&self, today: &NaiveDate) -> bool {
        let Some(aired) = self.aired else {
            return false;
        };

        aired <= *today
    }

    /// Get aired timestamp.
    pub(crate) fn aired_timestamp(&self) -> Option<DateTime<Utc>> {
        self.aired.as_ref().and_then(|&d| {
            Some(DateTime::from_naive_utc_and_offset(
                d.and_hms_opt(0, 0, 0)?,
                Utc,
            ))
        })
    }

    /// A sort key used for episodes.
    pub(crate) fn watch_order_key(&self) -> WatchOrderKey {
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
pub(crate) struct WatchOrderKey {
    /// Absolute number in the series.
    pub(crate) absolute_number: u32,
    /// Air date of the episode.
    pub(crate) aired: NaiveDate,
    /// Season number.
    pub(crate) season: SeasonNumber,
    /// Episode number inside of its season.
    pub(crate) number: u32,
}

/// The kind of a pending item.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum PendingKind {
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
pub(crate) struct Pending {
    pub(crate) timestamp: DateTime<Utc>,
    #[serde(flatten)]
    pub(crate) kind: PendingKind,
}

impl Pending {
    /// Access the raw id for the pending item.
    pub(crate) fn id(&self) -> &Uuid {
        match &self.kind {
            PendingKind::Episode { series, .. } => series.id(),
            PendingKind::Movie { movie } => movie.id(),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SearchSeries {
    pub(crate) id: RemoteId,
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
    pub(crate) id: RemoteId,
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
