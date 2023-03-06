use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use bytes::Bytes;
use chrono::{DateTime, NaiveDate, Utc};
use relative_path::RelativePath;
use reqwest::{Method, RequestBuilder, Response, Url};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::api::common;
use crate::model::{
    Episode, EpisodeGraphics, EpisodeId, Etag, ImageV2, Raw, RemoteEpisodeId, RemoteSeriesId,
    SearchSeries, SeasonNumber, Series, SeriesGraphics, SeriesId,
};
use crate::service::NewEpisode;

const BASE_URL: &str = "https://api.thetvdb.com";
const ARTWORKS_URL: &str = "https://artworks.thetvdb.com";
const EXPIRATION_SECONDS: u64 = 3600;
const IDLE_TIMEOUT: Duration = Duration::from_secs(10);

struct Credentials {
    token: Box<str>,
    expires_at: Instant,
}

impl Credentials {
    fn is_expired(&self) -> bool {
        Instant::now() < self.expires_at
    }
}

struct State {
    // NB: using tokio sync mutex to ensure only one client at a time is
    // attempting to login.
    cached: tokio::sync::Mutex<Option<Credentials>>,
    base_url: Url,
    artworks_url: Url,
}

#[derive(Clone)]
pub(crate) struct Client {
    state: Arc<State>,
    client: reqwest::Client,
    api_key: Arc<str>,
}

impl Client {
    /// Construct a new client wrapping the given api key.
    pub(crate) fn new<S>(api_key: &S) -> Result<Self>
    where
        S: ?Sized + AsRef<str>,
    {
        Ok(Self {
            state: Arc::new(State {
                cached: tokio::sync::Mutex::new(None),
                base_url: Url::parse(BASE_URL).expect("illegal base url"),
                artworks_url: Url::parse(ARTWORKS_URL).expect("illegal artworks url"),
            }),
            client: reqwest::ClientBuilder::new()
                .pool_idle_timeout(IDLE_TIMEOUT)
                .build()?,
            api_key: api_key.as_ref().into(),
        })
    }

    /// Set API key to the given value.
    pub(crate) fn set_api_key<S>(&mut self, api_key: &S)
    where
        S: ?Sized + AsRef<str>,
    {
        self.api_key = api_key.as_ref().into();
    }

    fn request<I>(&self, method: Method, segments: I) -> RequestBuilder
    where
        I: IntoIterator,
        I::Item: AsRef<str>,
    {
        let mut url = self.state.base_url.clone();

        if let Ok(mut m) = url.path_segments_mut() {
            m.extend(segments);
        }

        self.client
            .request(method, url)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
    }

    /// Login with the current API key.
    async fn login(&self) -> Result<Box<str>> {
        let mut cached = self.state.cached.lock().await;

        if let Some(c) = &*cached {
            if !c.is_expired() {
                tracing::debug!("using cached credentials");
                return Ok(c.token.clone());
            }
        }

        let req = self
            .request(Method::POST, &["login"])
            .json(&Body {
                apikey: &self.api_key,
            })
            .build()?;

        let res = self.client.execute(req).await?;
        let res: Bytes = handle_res(res).await?;
        let res: Response = serde_json::from_slice(&res)?;

        let expires_at = Instant::now()
            .checked_add(Duration::from_secs(EXPIRATION_SECONDS))
            .context("instant overflow")?;

        *cached = Some(Credentials {
            token: res.token.clone().into(),
            expires_at,
        });

        return Ok(res.token.into());

        #[derive(Serialize)]
        struct Body<'a> {
            apikey: &'a str,
        }

        #[derive(Deserialize)]
        struct Response {
            token: String,
        }
    }

    /// Request with (hopefully cached) authorization.
    async fn request_with_auth<I>(&self, method: Method, segments: I) -> Result<RequestBuilder>
    where
        I: IntoIterator,
        I::Item: AsRef<str>,
    {
        let token = self.login().await?;
        Ok(self.request(method, segments).bearer_auth(&token))
    }

    /// Get last modified timestamp of a series.
    pub(crate) async fn series_last_modified(&self, id: u32) -> Result<Option<DateTime<Utc>>> {
        let res = self
            .request_with_auth(Method::HEAD, &["series", &id.to_string()])
            .await?
            .send()
            .await?;

        common::parse_last_modified(&res).context("last-modified header")
    }

    /// Download series information.
    pub(crate) async fn series(
        &self,
        id: u32,
        lookup: impl common::LookupSeriesId,
    ) -> Result<(
        Series,
        BTreeSet<RemoteSeriesId>,
        Option<Etag>,
        Option<DateTime<Utc>>,
    )> {
        let res = self
            .request_with_auth(Method::GET, &["series", &id.to_string()])
            .await?
            .send()
            .await?;

        let last_etag = common::parse_etag(&res);
        let last_modified = common::parse_last_modified(&res).context("last-modified header")?;
        let bytes: Bytes = handle_res(res).await?;

        if tracing::enabled!(tracing::Level::TRACE) {
            let raw = serde_json::from_slice::<serde_json::Value>(&bytes)?;
            tracing::trace!("series: {raw}");
        }

        let value: Value = serde_json::from_slice::<Data<_>>(&bytes)?.data;

        let mut graphics = SeriesGraphics::default();
        graphics.banner = value.banner.as_deref().and_then(ImageV2::tvdb);
        graphics.fanart = value.fanart.as_deref().and_then(ImageV2::tvdb);
        graphics.poster = value.poster.as_deref().and_then(ImageV2::tvdb);

        let remote_id = RemoteSeriesId::Tvdb { id };

        let mut remote_ids = BTreeSet::from([remote_id]);

        if let Some(imdb_id) = value.imdb_id.filter(|id| !id.is_empty()) {
            remote_ids.insert(RemoteSeriesId::Imdb {
                id: Raw::new(&imdb_id).context("id overflow")?,
            });
        }

        // Try to lookup the series by known remote ids.
        let id = lookup
            .lookup(remote_ids.iter().copied())
            .unwrap_or_else(SeriesId::random);

        let series = Series {
            id,
            title: value.series_name.to_owned(),
            first_air_date: None,
            overview: value.overview.unwrap_or_default(),
            compat_banner: None,
            compat_poster: None,
            compat_fanart: None,
            graphics,
            remote_id: Some(remote_id),
            tracked: true,
            compat_last_modified: None,
            compat_last_etag: None,
            compat_last_sync: BTreeMap::new(),
        };

        return Ok((series, remote_ids, last_etag, last_modified));

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(unused)]
        struct Value {
            id: u32,
            #[serde(default)]
            banner: Option<String>,
            #[serde(default)]
            fanart: Option<String>,
            #[serde(default)]
            overview: Option<String>,
            #[serde(default)]
            poster: Option<String>,
            #[serde(default)]
            series_name: String,
            #[serde(default)]
            airs_day_of_week: Option<String>,
            #[serde(default)]
            airs_time: Option<String>,
            #[serde(default)]
            imdb_id: Option<String>,
        }
    }

    /// Download all series episodes.
    pub(crate) async fn series_episodes(
        &self,
        id: u32,
        lookup: impl common::LookupEpisodeId,
    ) -> Result<Vec<NewEpisode>> {
        let path = ["series", &id.to_string(), "episodes"];

        return self
            .paged_request("episode", &path, move |row: Row| {
                let mut graphics = EpisodeGraphics::default();
                graphics.filename = row.filename.as_deref().and_then(ImageV2::tvdb);

                let remote_id = RemoteEpisodeId::Tvdb { id: row.id };
                let mut remote_ids = BTreeSet::from([remote_id]);

                if let Some(imdb_id) = row.imdb_id.filter(|id| !id.is_empty()) {
                    remote_ids.insert(RemoteEpisodeId::Imdb {
                        id: Raw::new(&imdb_id).context("id overflow")?,
                    });
                }

                let id = lookup
                    .lookup(remote_ids.iter().copied())
                    .unwrap_or_else(EpisodeId::random);

                let episode = Episode {
                    id,
                    name: row.episode_name,
                    overview: row.overview.unwrap_or_default(),
                    absolute_number: row.absolute_number,
                    // NB: thetvdb.com uses season 0 as specials season.
                    season: match row.aired_season {
                        Some(n) if n > 0 => SeasonNumber::Number(n),
                        _ => SeasonNumber::Specials,
                    },
                    number: row.aired_episode_number,
                    aired: row.first_aired,
                    compat_filename: None,
                    graphics,
                    remote_id: Some(remote_id),
                };

                Ok(NewEpisode {
                    episode,
                    remote_ids,
                })
            })
            .await;

        #[derive(Debug, Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(unused)]
        struct Row {
            id: u32,
            #[serde(default)]
            absolute_number: Option<u32>,
            aired_episode_number: u32,
            #[serde(default)]
            aired_season: Option<u32>,
            #[serde(default)]
            episode_name: Option<String>,
            #[serde(default)]
            overview: Option<String>,
            #[serde(default)]
            filename: Option<String>,
            #[serde(default)]
            first_aired: Option<NaiveDate>,
            #[serde(default)]
            imdb_id: Option<String>,
        }
    }

    /// Handle series pagination.
    async fn paged_request<T, U, M, I>(
        &self,
        thing: &'static str,
        path: I,
        mut map: M,
    ) -> Result<Vec<U>>
    where
        T: DeserializeOwned + fmt::Debug,
        M: FnMut(T) -> Result<U>,
        I: Copy + IntoIterator,
        I::Item: AsRef<str>,
    {
        let res = self
            .request_with_auth(Method::GET, path)
            .await?
            .send()
            .await?;

        let bytes: Bytes = handle_res(res).await?;

        if tracing::enabled!(tracing::Level::TRACE) {
            let raw = serde_json::from_slice::<serde_json::Value>(&bytes)?;
            tracing::trace!("paged: {raw}");
        }

        let mut data: DataLinks<Vec<serde_json::Value>> = serde_json::from_slice(&bytes)?;

        let mut output = Vec::new();

        loop {
            output.reserve(data.data.len());

            for value in data.data {
                tracing::trace!("{}: {thing}: {value}", output.len());

                let row = match serde_json::from_value::<T>(value) {
                    Ok(row) => row,
                    Err(error) => {
                        tracing::warn!("{}: {thing}: {error}", output.len());
                        continue;
                    }
                };

                tracing::trace!("{}: {thing}: {row:?}", output.len());
                output.push(map(row)?);
            }

            let Some(next) = data.links.next else {
                break;
            };

            let res = self
                .request_with_auth(Method::GET, path)
                .await?
                .query(&[("page", &next.to_string())])
                .send()
                .await?;

            let bytes: Bytes = handle_res(res).await?;

            if tracing::enabled!(tracing::Level::TRACE) {
                let raw = serde_json::from_slice::<serde_json::Value>(&bytes)?;
                tracing::trace!("{raw}");
            }

            data = serde_json::from_slice(&bytes)?;
        }

        Ok(output)
    }

    /// Search series result.
    pub(crate) async fn search_by_name(&self, name: &str) -> Result<Vec<SearchSeries>> {
        let res = self
            .request_with_auth(Method::GET, &["search", "series"])
            .await?
            .query(&[&("name", name)])
            .send()
            .await?;

        let bytes: Bytes = handle_res(res).await?;

        if tracing::enabled!(tracing::Level::TRACE) {
            let raw = serde_json::from_slice::<serde_json::Value>(&bytes)?;
            tracing::trace!("search_by_name: {raw}");
        }

        let data: Data<Vec<serde_json::Value>> = serde_json::from_slice(&bytes)?;

        let mut output = Vec::with_capacity(data.data.len());

        for (index, row) in data.data.into_iter().enumerate() {
            let row: Row = match serde_json::from_value(row) {
                Ok(row) => row,
                Err(error) => {
                    tracing::error!("#{index}: {error}");
                    continue;
                }
            };

            let poster = row.poster.as_deref().and_then(ImageV2::tvdb);

            output.push(SearchSeries {
                id: RemoteSeriesId::Tvdb { id: row.id },
                name: row.series_name,
                poster,
                overview: row.overview.unwrap_or_default(),
                first_aired: row.first_aired,
            });
        }

        return Ok(output);

        #[derive(Debug, Clone, Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub(crate) struct Row {
            pub(crate) id: u32,
            #[serde(default)]
            pub(crate) series_name: String,
            #[serde(default)]
            pub(crate) poster: Option<String>,
            #[serde(default)]
            pub(crate) overview: Option<String>,
            #[serde(default)]
            pub(crate) first_aired: Option<NaiveDate>,
        }
    }

    /// Load image data from image path.
    pub(crate) async fn download_image_path(&self, path: &RelativePath) -> Result<Vec<u8>> {
        let mut url = self.state.artworks_url.clone();

        if let Ok(mut segments) = url.path_segments_mut() {
            segments.extend(["banners"]);

            for c in path.components() {
                segments.push(c.as_str());
            }
        }

        let res = self.client.get(url).send().await?;

        if !res.status().is_success() {
            bail!("{path}: failed to download image: {}", res.status());
        }

        Ok(res.bytes().await?.to_vec())
    }
}

/// Handle converting response to JSON.
async fn handle_res(res: Response) -> Result<Bytes> {
    if !res.status().is_success() {
        bail!("{}: {}", res.status(), res.text().await?);
    }

    Ok(res.bytes().await?)
}

#[derive(Deserialize)]
#[allow(unused)]
struct Links {
    first: u32,
    last: u32,
    #[serde(default)]
    next: Option<u32>,
    #[serde(default)]
    prev: Option<u32>,
}

#[derive(Deserialize)]
struct DataLinks<T> {
    data: T,
    links: Links,
}

#[derive(Deserialize)]
struct Data<T> {
    data: T,
}
