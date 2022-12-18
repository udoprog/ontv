use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use reqwest::{Method, Request, RequestBuilder, Response, Url};
use serde::{Deserialize, Serialize};

use crate::model::{Image, SearchSeries};

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
        let res: Response = to_json(res).await?;

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

    /// Search series result.
    pub(crate) async fn search_by_name(&self, name: &str) -> Result<Vec<SearchSeries>> {
        let res = self
            .request_with_auth(Method::GET, &["search", "series"])
            .await?
            .query(&[&("name", name)])
            .send()
            .await?;

        let data: Data<Row> = to_json(res).await?;

        let mut output = Vec::with_capacity(data.data.len());

        for row in data.data {
            let poster = Image::thetvdb_parse(&row.poster)?;

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
            pub(crate) id: u64,
            #[serde(rename = "seriesName")]
            pub(crate) name: String,
            #[serde(default)]
            pub(crate) poster: String,
            #[serde(default)]
            pub(crate) overview: String,
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
async fn to_json<T>(mut res: Response) -> Result<T>
where
    T: for<'de> Deserialize<'de>,
{
    if !res.status().is_success() {
        bail!("{}: {}", res.status(), res.text().await?);
    }

    Ok(res.json().await?)
}

#[derive(Deserialize)]
struct Data<T> {
    data: Vec<T>,
}
