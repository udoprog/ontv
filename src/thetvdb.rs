use std::fmt;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use bytes::Bytes;
use chrono::NaiveDate;
use reqwest::{Method, RequestBuilder, Response, Url};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::model::{
    Episode, Image, RemoteEpisodeId, RemoteSeriesId, SearchSeries, SeasonNumber, Series, SeriesId,
};

const BASE_URL: &str = "https://api.thetvdb.com";
const ARTWORKS_URL: &str = "https://artworks.thetvdb.com";
const EXPIRATION_SECONDS: u64 = 3600;

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
    pub(crate) fn new<S>(api_key: &S) -> Self
    where
        S: ?Sized + AsRef<str>,
    {
        Self {
            state: Arc::new(State {
                cached: tokio::sync::Mutex::new(None),
                base_url: Url::parse(BASE_URL).expect("illegal base url"),
                artworks_url: Url::parse(ARTWORKS_URL).expect("illegal artworks url"),
            }),
            client: reqwest::Client::new(),
            api_key: api_key.as_ref().into(),
        }
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
                log::debug!("using cached credentials");
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

    /// Download series information.
    pub(crate) async fn series(&self, id: SeriesId, new_id: Uuid) -> Result<Series> {
        let res = self
            .request_with_auth(Method::GET, &["series", &id.to_string()])
            .await?
            .send()
            .await?;

        let bytes: Bytes = handle_res(res).await?;

        if log::log_enabled!(log::Level::Trace) {
            let raw = serde_json::from_slice::<serde_json::Value>(&bytes)?;
            log::trace!("{raw}");
        }

        let value: Value = serde_json::from_slice::<Data<_>>(&bytes)?.data;

        let banner = match &value.banner {
            Some(banner) if !banner.is_empty() => {
                Some(Image::parse_banner(banner).context("banner image")?)
            }
            _ => None,
        };

        let fanart = match &value.fanart {
            Some(fanart) if !fanart.is_empty() => {
                Some(Image::parse_banner(fanart).context("fanart image")?)
            }
            _ => None,
        };

        let poster = Image::parse_banner(&value.poster).context("poster image")?;

        return Ok(Series {
            id: new_id,
            title: value.series_name.to_owned(),
            overview: value.overview,
            banner,
            poster,
            fanart,
            remote_ids: Vec::from([RemoteSeriesId::TheTvDb { id }]),
            tracked: true,
        });

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(unused)]
        struct Value {
            id: SeriesId,
            // "2021-03-05 07:53:14"
            added: String,
            banner: Option<String>,
            fanart: Option<String>,
            #[serde(default)]
            overview: Option<String>,
            poster: String,
            series_name: String,
            #[serde(default)]
            airs_day_of_week: Option<String>,
            #[serde(default)]
            airs_time: Option<String>,
        }
    }

    /// Download all series episodes.
    pub(crate) async fn series_episodes<A>(
        &self,
        id: SeriesId,
        mut alloc: A,
    ) -> Result<Vec<Episode>>
    where
        A: FnMut(SeriesId) -> Uuid,
    {
        let path = ["series", &id.to_string(), "episodes"];

        return self
            .paged_request("episode", &path, move |row: Row| {
                let filename = match row.filename {
                    Some(filename) if !filename.is_empty() => {
                        Some(Image::parse_banner(&filename).context("filename")?)
                    }
                    _ => None,
                };

                let id = alloc(row.id);

                Ok(Episode {
                    id,
                    name: row.episode_name,
                    overview: row.overview.filter(|o| !o.is_empty()),
                    absolute_number: row.absolute_number,
                    // NB: thetvdb.com uses season 0 as specials season.
                    season: match row.aired_season {
                        Some(0) => SeasonNumber::Specials,
                        Some(number) => SeasonNumber::Number(number),
                        None => SeasonNumber::Unknown,
                    },
                    number: row.aired_episode_number,
                    aired: row.first_aired,
                    filename,
                    remote_ids: Vec::from([RemoteEpisodeId::TheTvDb { id: row.id }]),
                })
            })
            .await;

        #[derive(Debug, Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(unused)]
        struct Row {
            id: SeriesId,
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

        if log::log_enabled!(log::Level::Trace) {
            let raw = serde_json::from_slice::<serde_json::Value>(&bytes)?;
            log::trace!("{raw}");
        }

        let mut data: DataLinks<Vec<serde_json::Value>> = serde_json::from_slice(&bytes)?;

        let mut output = Vec::new();

        loop {
            output.reserve(data.data.len());

            for value in data.data {
                log::trace!("{}: {thing}: {value}", output.len());

                let row = match serde_json::from_value::<T>(value) {
                    Ok(row) => row,
                    Err(error) => {
                        log::warn!("{}: {thing}: {error}", output.len());
                        continue;
                    }
                };

                log::trace!("{}: {thing}: {row:?}", output.len());
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

            if log::log_enabled!(log::Level::Trace) {
                let raw = serde_json::from_slice::<serde_json::Value>(&bytes)?;
                log::trace!("{raw}");
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

        let data: Bytes = handle_res(res).await?;
        let data: Data<Vec<Row>> = serde_json::from_slice(&data)?;

        let mut output = Vec::with_capacity(data.data.len());

        for row in data.data {
            let poster = Image::parse(&row.poster)?;

            output.push(SearchSeries {
                id: row.id,
                name: row.name,
                poster,
                overview: row.overview,
            });
        }

        return Ok(output);

        #[derive(Debug, Clone, Deserialize)]
        pub(crate) struct Row {
            pub(crate) id: SeriesId,
            #[serde(rename = "seriesName")]
            pub(crate) name: String,
            #[serde(default)]
            pub(crate) poster: String,
            #[serde(default)]
            pub(crate) overview: Option<String>,
        }
    }

    /// Load image data.
    pub(crate) async fn get_image_data(&self, id: &Image) -> Result<Vec<u8>> {
        let mut url = self.state.artworks_url.clone();
        url.set_path(&id.to_string());
        let res = self.client.get(url).send().await?;
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
