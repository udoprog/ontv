mod hex16;
mod raw16;

use std::collections::BTreeMap;
use std::fmt;

use anyhow::{anyhow, bail, ensure, Context, Result};
use chrono::{DateTime, NaiveDate, Utc};
use iced::widget::{text, Text};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub(crate) use self::hex16::Hex16;
pub(crate) use self::raw16::Raw16;

#[derive(Default, Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ThemeType {
    #[default]
    Light,
    Dark,
}

/// The state for the settings page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Config {
    #[serde(default)]
    pub(crate) theme: ThemeType,
    #[serde(default)]
    pub(crate) thetvdb_legacy_apikey: String,
}

impl Default for Config {
    #[inline]
    fn default() -> Self {
        Self {
            theme: ThemeType::Dark,
            thetvdb_legacy_apikey: String::new(),
        }
    }
}

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
    Imdb { id: Raw16 },
}

impl fmt::Display for RemoteSeriesId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RemoteSeriesId::TheTvDb { id } => {
                write!(f, "thetvdb.com ({id})")
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
    TheTvDb { id: SeriesId },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(transparent)]
#[serde(transparent)]
pub(crate) struct SeriesId(u64);

impl From<u32> for SeriesId {
    #[inline]
    fn from(value: u32) -> Self {
        Self(value as u64)
    }
}

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
    /// Overview of the series.
    #[serde(default)]
    pub(crate) overview: Option<String>,
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
    /// Locally known last modified timestamp.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) last_modified: Option<DateTime<Utc>>,
    /// Last sync time for each remote.
    #[serde(
        default,
        skip_serializing_if = "BTreeMap::is_empty",
        with = "btree_as_vec"
    )]
    pub(crate) last_sync: BTreeMap<RemoteSeriesId, DateTime<Utc>>,
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
#[derive(Default, Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) struct Watched {
    /// Unique identifier for this watch.
    pub(crate) id: Uuid,
    /// Identifier of watched series.
    pub(crate) series: Uuid,
    /// Identifier of watched episode.
    pub(crate) episode: Uuid,
    /// Timestamp when it was watched.
    pub(crate) timestamp: DateTime<Utc>,
}

/// Season number.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SeasonNumber {
    Number(u32),
    #[serde(rename = "specials")]
    Specials,
    #[serde(rename = "unknown")]
    #[default]
    Unknown,
}

impl SeasonNumber {
    /// Build season title.
    pub(crate) fn title(&self) -> Text<'static> {
        match self {
            SeasonNumber::Number(number) => text(format!("Season {}", number)),
            SeasonNumber::Specials => text("Specials"),
            SeasonNumber::Unknown => text("N/A"),
        }
    }

    /// Build season title.
    pub(crate) fn short(&self) -> Text<'static> {
        match self {
            SeasonNumber::Number(number) => text(format!("S{}", number)),
            SeasonNumber::Specials => text("S"),
            SeasonNumber::Unknown => text("N/A"),
        }
    }
}

/// A season in a series.
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Season {
    /// The number of the season.
    #[serde(default)]
    pub(crate) number: SeasonNumber,
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
    /// Remote episode ids.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) remote_ids: Vec<RemoteEpisodeId>,
}

impl Episode {
    /// Test if the given episode has aired by the provided timestamp.
    pub(crate) fn has_aired(&self, now: &DateTime<Utc>) -> bool {
        let Some(aired) = &self.aired else {
            return false;
        };

        *aired <= now.date_naive()
    }
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
                bail!("unsupported image format")
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

        Self::parse_banner_it(it).with_context(|| anyhow!("bad image: {input}"))
    }

    /// Parse without expecting a `banners` prefix.
    #[inline]
    pub(crate) fn parse_banner(input: &str) -> Result<Self> {
        Self::parse_banner_it(input.split('/')).with_context(|| anyhow!("bad image: {input}"))
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

        let format = ImageFormat::parse(ext)?;

        let mut array = ArrayVec::<_, 6>::new();

        for part in it {
            array.try_push(part).map_err(|e| anyhow!("{e}"))?;
        }

        array.try_push(rest).map_err(|e| anyhow!("{e}"))?;

        let kind = match &array[..] {
            // blank/77092.jpg
            &["blank", series_id] => ImageKind::Blank(series_id.parse()?),
            // images/missing/series.jpg
            &["images", "missing", "series"] => ImageKind::Missing,
            &["v4", "series", series_id, kind, rest] => {
                let kind = ArtKind::parse(kind)?;
                let id = Hex16::from_hex(rest).context("bad id")?;
                ImageKind::V4(series_id.parse()?, kind, id)
            }
            &["series", series_id, kind, id] => {
                let series_id = series_id.parse()?;
                let kind = ArtKind::parse(kind)?;
                let id = Hex16::from_hex(id).context("bad id")?;
                ImageKind::Legacy(series_id, kind, id)
            }
            &["posters", rest] => {
                if let Some((series_id, suffix)) = rest.split_once('-') {
                    let series_id = series_id.parse()?;
                    let suffix = Raw16::from_string(suffix);
                    ImageKind::BannerSuffixed(series_id, suffix)
                } else {
                    let id = Hex16::from_hex(rest).context("bad id")?;
                    ImageKind::Banner(id)
                }
            }
            &["graphical", rest] => {
                if let Some((series_id, suffix)) = rest.split_once('-') {
                    let series_id = series_id.parse()?;
                    let suffix = Raw16::from_string(suffix);
                    ImageKind::GraphicalSuffixed(series_id, suffix)
                } else {
                    let id = Hex16::from_hex(rest).context("bad hex")?;
                    ImageKind::Graphical(id)
                }
            }
            &["fanart", "original", rest] => {
                if let Some((series_id, suffix)) = rest.split_once('-') {
                    let series_id = series_id.parse()?;
                    let suffix = Raw16::from_string(suffix);
                    ImageKind::FanartSuffixed(series_id, suffix)
                } else {
                    let id = Hex16::from_hex(rest).context("bad hex")?;
                    ImageKind::Fanart(id)
                }
            }
            // Example: v4/episode/8538342/screencap/63887bf74c84e.jpg
            &["v4", "episode", episode_id, "screencap", rest] => {
                let id = Hex16::from_hex(rest).context("bad id")?;
                ImageKind::ScreenCap(episode_id.parse()?, id)
            }
            &["episodes", episode_id, rest] => {
                ImageKind::Episodes(episode_id.parse()?, rest.parse()?)
            }
            _ => {
                bail!("unsupported image");
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
pub(crate) enum ImageKind {
    Legacy(u64, ArtKind, Hex16),
    V4(u64, ArtKind, Hex16),
    Banner(Hex16),
    BannerSuffixed(u64, Raw16),
    Graphical(Hex16),
    GraphicalSuffixed(u64, Raw16),
    Fanart(Hex16),
    FanartSuffixed(u64, Raw16),
    ScreenCap(u64, Hex16),
    Episodes(u32, u32),
    Blank(u32),
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
            ImageKind::Graphical(id) => {
                write!(f, "/banners/graphical/{id}.{format}")
            }
            ImageKind::GraphicalSuffixed(series_id, suffix) => {
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
            ImageKind::Blank(series_id) => {
                write!(f, "/banners/blank/{series_id}.{format}")
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
    pub(crate) poster: Option<Image>,
    pub(crate) overview: Option<String>,
    pub(crate) first_aired: Option<NaiveDate>,
}
