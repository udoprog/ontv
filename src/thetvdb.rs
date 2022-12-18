use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use bytes::Bytes;
use reqwest::{Method, RequestBuilder, Response, Url};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::model::{Image, RemoteSeriesId, SearchSeries, Series, Source, TheTvDbSeriesId};

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
    api_key: Mutex<Box<str>>,
}

#[derive(Clone)]
pub(crate) struct Client {
    state: Arc<State>,
    client: reqwest::Client,
}

impl Client {
    /// Construct a new client wrapping the given api key.
    pub(crate) fn new() -> Self {
        Self {
            state: Arc::new(State {
                cached: tokio::sync::Mutex::new(None),
                base_url: Url::parse(BASE_URL).expect("illegal base url"),
                artworks_url: Url::parse(ARTWORKS_URL).expect("illegal artworks url"),
                api_key: Mutex::new("".into()),
            }),
            client: reqwest::Client::new(),
        }
    }

    /// Set API key to the given value.
    pub(crate) fn set_api_key<S>(&self, api_key: &S)
    where
        S: ?Sized + AsRef<str>,
    {
        *self.state.api_key.lock().unwrap() = api_key.as_ref().into();
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

        let req = {
            let api_key = self.state.api_key.lock().unwrap();

            self.request(Method::POST, &["login"])
                .json(&Body { apikey: &api_key })
                .build()?
        };

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
    pub(crate) async fn series(&self, id: TheTvDbSeriesId) -> Result<Series> {
        let res = self
            .request_with_auth(Method::GET, &["series", &id.to_string()])
            .await?
            .send()
            .await?;

        let raw: Bytes = handle_res(res).await?;
        log::trace!("{}", serde_json::from_slice::<serde_json::Value>(&raw)?);
        let value: Value = serde_json::from_slice::<Data<_>>(&raw)?.data;

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
            id: Uuid::new_v4(),
            title: value.series_name.to_owned(),
            banner,
            poster,
            fanart,
            remote_ids: Vec::from([RemoteSeriesId::TheTvDb { id }]),
            raw: [(Source::TheTvDb, raw)].into_iter().collect(),
        });

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(unused)]
        struct Value {
            id: TheTvDbSeriesId,
            // "2021-03-05 07:53:14"
            added: String,
            banner: Option<String>,
            fanart: Option<String>,
            overview: Option<String>,
            poster: String,
            series_name: String,
        }
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
            pub(crate) id: TheTvDbSeriesId,
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
struct Data<T> {
    data: T,
}
