mod etag;
mod hex;
mod raw;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::str::FromStr;

use anyhow::{anyhow, bail, ensure, Context, Result};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub(crate) use self::etag::Etag;
pub(crate) use self::hex::Hex;
pub(crate) use self::raw::Raw;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(transparent)]
#[serde(transparent)]
pub(crate) struct SeriesId(Uuid);

impl SeriesId {
    /// Generate a new random series identifier.
    #[inline]
    pub(crate) fn random() -> Self {
        Self(Uuid::new_v4())
    }
}

impl fmt::Display for SeriesId {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for SeriesId {
    type Err = uuid::Error;

    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(uuid::Uuid::from_str(s)?))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(transparent)]
#[serde(transparent)]
pub(crate) struct EpisodeId(Uuid);

impl EpisodeId {
    /// Generate a new random episode identifier.
    #[inline]
    pub(crate) fn random() -> Self {
        Self(Uuid::new_v4())
    }
}

impl fmt::Display for EpisodeId {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for EpisodeId {
    type Err = uuid::Error;

    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(uuid::Uuid::from_str(s)?))
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
        remotes: BTreeSet<RemoteSeriesId>,
    },
    Episode {
        uuid: EpisodeId,
        remotes: BTreeSet<RemoteEpisodeId>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "remote")]
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
                write!(f, "thetvdb.com ({id})")
            }
            RemoteSeriesId::Tmdb { id } => {
                write!(f, "themoviedb.org ({id})")
            }
            RemoteSeriesId::Imdb { id } => {
                write!(f, "imdb.com ({id})")
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "remote")]
pub(crate) enum RemoteEpisodeId {
    Tvdb { id: u32 },
    Tmdb { id: u32 },
    Imdb { id: Raw<16> },
}

/// A series.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct Series {
    /// Allocated UUID.
    pub(crate) id: SeriesId,
    /// Title of the series.
    pub(crate) title: String,
    /// First air date of the series.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) first_air_date: Option<NaiveDate>,
    /// Overview of the series.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) overview: Option<String>,
    /// Poster image.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) poster: Option<Image>,
    /// Banner image.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) banner: Option<Image>,
    /// Fanart image.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) fanart: Option<Image>,
    /// Indicates if the series is tracked or not, in that it will receive updates.
    #[serde(default, skip_serializing_if = "is_false")]
    pub(crate) tracked: bool,
    /// Locally known last modified timestamp.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) last_modified: Option<DateTime<Utc>>,
    /// Locally known last etag.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) last_etag: Option<Etag>,
    /// Last sync time for each remote.
    #[serde(
        default,
        skip_serializing_if = "BTreeMap::is_empty",
        with = "btree_as_vec"
    )]
    pub(crate) last_sync: BTreeMap<RemoteSeriesId, DateTime<Utc>>,
    /// The remote identifier that is used to synchronize this series.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) remote_id: Option<RemoteSeriesId>,
    /// Remote series ids.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) remote_ids: Vec<RemoteSeriesId>,
}

#[inline]
fn is_false(b: &bool) -> bool {
    !*b
}

mod btree_as_vec {
    use std::collections::BTreeMap;
    use std::fmt;

    use serde::de;
    use serde::ser;
    use serde::ser::SerializeSeq;

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
            write!(f, "expected sequence")
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
#[serde(rename_all = "snake_case")]
pub(crate) struct Watched {
    /// Unique identifier for this watch.
    pub(crate) id: Uuid,
    /// Identifier of watched series.
    pub(crate) series: SeriesId,
    /// Identifier of watched episode.
    pub(crate) episode: EpisodeId,
    /// Timestamp when it was watched.
    pub(crate) timestamp: DateTime<Utc>,
}

/// Season number.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
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
    fn is_special(&self) -> bool {
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

/// A season in a series.
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct Season {
    /// The number of the season.
    #[serde(default, skip_serializing_if = "SeasonNumber::is_special")]
    pub(crate) number: SeasonNumber,
    #[serde(default)]
    pub(crate) air_date: Option<NaiveDate>,
    #[serde(default)]
    pub(crate) name: Option<String>,
    #[serde(default)]
    pub(crate) overview: Option<String>,
    #[serde(default)]
    pub(crate) poster: Option<Image>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) overview: Option<String>,
    /// Absolute number in the series.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) absolute_number: Option<u32>,
    /// Season number.
    #[serde(default)]
    pub(crate) season: SeasonNumber,
    /// Episode number inside of its season.
    pub(crate) number: u32,
    /// Air date of the episode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) aired: Option<NaiveDate>,
    /// Episode image.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) filename: Option<Image>,
    /// The remote identifier that is used to synchronize this episode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) remote_id: Option<RemoteEpisodeId>,
    /// Remote episode ids.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub(crate) remote_ids: BTreeSet<RemoteEpisodeId>,
}

impl Episode {
    /// Test if the given episode has aired by the provided timestamp.
    pub(crate) fn has_aired(&self, today: &NaiveDate) -> bool {
        let Some(aired) = &self.aired else {
            return false;
        };

        *aired <= *today
    }
}

impl fmt::Display for Episode {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} / {}", self.season, self.number)
    }
}

/// Image format in use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ImageExt {
    Jpg,
}

impl ImageExt {
    /// Parse a banner URL.
    fn parse(input: &str) -> Result<Self> {
        match input {
            "jpg" => Ok(ImageExt::Jpg),
            _ => {
                bail!("unsupported image format")
            }
        }
    }
}

impl fmt::Display for ImageExt {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImageExt::Jpg => write!(f, "jpg"),
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

impl ArtKind {
    fn parse(input: &str) -> Result<Self> {
        match input {
            "posters" => Ok(ArtKind::Posters),
            "banners" => Ok(ArtKind::Banners),
            "backgrounds" => Ok(ArtKind::Backgrounds),
            "episodes" => Ok(ArtKind::Episodes),
            _ => {
                bail!("unsupported art kind")
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
                write!(f, "/banners/series/{series_id}/{kind}/{id}.{ext}")
            }
            TvdbImageKind::V4(series_id, kind, id) => {
                write!(f, "/banners/v4/series/{series_id}/{kind}/{id}.{ext}")
            }
            TvdbImageKind::Banner(id) => {
                write!(f, "/banners/posters/{id}.{ext}")
            }
            TvdbImageKind::BannerSuffixed(series_id, suffix) => {
                write!(f, "/banners/posters/{series_id}-{suffix}.{ext}")
            }
            TvdbImageKind::Graphical(id) => {
                write!(f, "/banners/graphical/{id}.{ext}")
            }
            TvdbImageKind::GraphicalSuffixed(series_id, suffix) => {
                write!(f, "/banners/graphical/{series_id}-{suffix}.{ext}")
            }
            TvdbImageKind::Fanart(id) => {
                write!(f, "/banners/fanart/original/{id}.{ext}")
            }
            TvdbImageKind::FanartSuffixed(series_id, suffix) => {
                write!(f, "/banners/fanart/original/{series_id}-{suffix}.{ext}")
            }
            TvdbImageKind::ScreenCap(episode_id, id) => {
                write!(f, "/banners/v4/episode/{episode_id}/screencap/{id}.{ext}")
            }
            TvdbImageKind::Episodes(episode_id, image_id) => {
                write!(f, "/banners/episodes/{episode_id}/{image_id}.{ext}")
            }
            TvdbImageKind::Blank(series_id) => {
                write!(f, "/banners/blank/{series_id}.{ext}")
            }
            TvdbImageKind::Missing => {
                write!(f, "/banners/images/missing/series.{ext}")
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
                write!(f, "/t/p/original/{id}.{ext}")?;
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
    /// Parse an image URL from thetvdb.
    pub(crate) fn parse_tvdb(mut input: &str) -> Result<Self> {
        input = input.trim_start_matches('/');

        let mut it = input.split('/');

        ensure!(
            matches!(it.next(), Some("banners")),
            "{input}: missing `banners`"
        );

        Self::parse_banner_it(it).with_context(|| anyhow!("bad image: {input}"))
    }

    /// Parse without expecting a `banners` prefix.
    #[inline]
    pub(crate) fn parse_tvdb_banner(input: &str) -> Result<Self> {
        Self::parse_banner_it(input.split('/')).with_context(|| anyhow!("bad image: {input}"))
    }

    #[inline]
    pub(crate) fn parse_tmdb(input: &str) -> Result<Self> {
        let input = input.trim_start_matches('/');

        let Some((id, ext)) = input.split_once('.') else {
            bail!("missing extension");
        };

        let id = Raw::new(id).context("base identifier")?;
        let kind = TmdbImageKind::Base64(id);
        let ext = ImageExt::parse(ext)?;
        Ok(Image::Tmdb(TmdbImage { kind, ext }))
    }

    fn parse_banner_it<'a, I>(mut it: I) -> Result<Self>
    where
        I: DoubleEndedIterator<Item = &'a str>,
    {
        use arrayvec::ArrayVec;

        let rest = it.next_back().context("missing last component")?;

        let Some((rest, ext)) = rest.split_once('.') else {
            bail!("missing extension");
        };

        let ext = ImageExt::parse(ext)?;

        let mut array = ArrayVec::<_, 6>::new();

        for part in it {
            array.try_push(part).map_err(|e| anyhow!("{e}"))?;
        }

        array.try_push(rest).map_err(|e| anyhow!("{e}"))?;

        let kind = match &array[..] {
            // blank/77092.jpg
            ["blank", series_id] => TvdbImageKind::Blank(series_id.parse()?),
            // images/missing/series.jpg
            ["images", "missing", "series"] => TvdbImageKind::Missing,
            ["v4", "series", series_id, kind, rest] => {
                let kind = ArtKind::parse(kind)?;
                let id = Hex::from_hex(rest).context("bad id")?;
                TvdbImageKind::V4(series_id.parse()?, kind, id)
            }
            ["series", series_id, kind, id] => {
                let series_id = series_id.parse()?;
                let kind = ArtKind::parse(kind)?;
                let id = Hex::from_hex(id).context("bad id")?;
                TvdbImageKind::Legacy(series_id, kind, id)
            }
            ["posters", rest] => {
                if let Some((series_id, suffix)) = rest.split_once('-') {
                    let series_id = series_id.parse()?;
                    let suffix = Raw::new(suffix).context("suffix overflow")?;
                    TvdbImageKind::BannerSuffixed(series_id, suffix)
                } else {
                    let id = Hex::from_hex(rest).context("bad id")?;
                    TvdbImageKind::Banner(id)
                }
            }
            ["graphical", rest] => {
                if let Some((series_id, suffix)) = rest.split_once('-') {
                    let series_id = series_id.parse()?;
                    let suffix = Raw::new(suffix).context("suffix overflow")?;
                    TvdbImageKind::GraphicalSuffixed(series_id, suffix)
                } else {
                    let id = Hex::from_hex(rest).context("bad hex")?;
                    TvdbImageKind::Graphical(id)
                }
            }
            ["fanart", "original", rest] => {
                if let Some((series_id, suffix)) = rest.split_once('-') {
                    let series_id = series_id.parse()?;
                    let suffix = Raw::new(suffix).context("suffix overflow")?;
                    TvdbImageKind::FanartSuffixed(series_id, suffix)
                } else {
                    let id = Hex::from_hex(rest).context("bad hex")?;
                    TvdbImageKind::Fanart(id)
                }
            }
            // Example: v4/episode/8538342/screencap/63887bf74c84e.jpg
            ["v4", "episode", episode_id, "screencap", rest] => {
                let id = Hex::from_hex(rest).context("bad id")?;
                TvdbImageKind::ScreenCap(episode_id.parse()?, id)
            }
            ["episodes", episode_id, rest] => {
                TvdbImageKind::Episodes(episode_id.parse()?, rest.parse()?)
            }
            _ => {
                bail!("unsupported image");
            }
        };

        Ok(Image::Tvdb(TvdbImage { kind, ext }))
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub(crate) enum TaskKind {
    /// Check for updates.
    CheckForUpdates {
        series_id: SeriesId,
        remote_id: RemoteSeriesId,
    },
    /// Task to download series data.
    DownloadSeriesById { series_id: SeriesId },
    /// Task to add a series by a remote identifier.
    DownloadSeriesByRemoteId { remote_id: RemoteSeriesId },
}

/// Actions that can be performed after a task has completed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "kebab-case")]
pub(crate) enum TaskFinished {
    /// Update series.
    SeriesSynced {
        /// Series to update.
        series_id: SeriesId,
        /// Remote series id.
        remote_id: RemoteSeriesId,
        /// Update last modified date.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        last_modified: Option<DateTime<Utc>>,
    },
}

/// A task in a queue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[must_use]
pub(crate) struct Task {
    /// The identifier of the task.
    pub(crate) id: Uuid,
    /// The kind of the task.
    #[serde(flatten)]
    pub(crate) kind: TaskKind,
    /// When the task is scheduled for.
    pub(crate) scheduled: DateTime<Utc>,
    /// Task finished actions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) finished: Option<TaskFinished>,
}

impl Task {
    /// Test if task involves the given series.
    pub(crate) fn is_series(&self, id: &SeriesId) -> bool {
        match &self.kind {
            TaskKind::DownloadSeriesById { series_id, .. } => *series_id == *id,
            TaskKind::CheckForUpdates { series_id, .. } => *series_id == *id,
            TaskKind::DownloadSeriesByRemoteId { .. } => false,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SearchSeries {
    pub(crate) id: RemoteSeriesId,
    pub(crate) name: String,
    pub(crate) poster: Option<Image>,
    pub(crate) overview: Option<String>,
    pub(crate) first_aired: Option<NaiveDate>,
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
