use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Result};
use bytes::Bytes;
use reqwest::{Method, RequestBuilder, Response, Url};
use serde::Deserialize;

use crate::model::{Image, RemoteSeriesId, SearchSeries, TmdbImage};

const BASE_URL: &str = "https://api.themoviedb.org/3";
const IMAGE_URL: &str = "https://image.tmdb.org";
const IDLE_TIMEOUT: Duration = Duration::from_secs(10);

struct State {
    base_url: Url,
    image_url: Url,
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
                base_url: Url::parse(BASE_URL).expect("illegal base url"),
                image_url: Url::parse(IMAGE_URL).expect("illegal artworks url"),
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

    /// Request with (hopefully cached) authorization.
    fn request_with_auth<I>(&self, method: Method, segments: I) -> RequestBuilder
    where
        I: IntoIterator,
        I::Item: AsRef<str>,
    {
        self.request(method, segments)
            .query(&[("api_key", self.api_key.as_ref())])
    }

    /// Search series result.
    pub(crate) async fn search_series(&self, query: &str) -> Result<Vec<SearchSeries>> {
        let res = self
            .request_with_auth(Method::GET, &["search", "tv"])
            .query(&[&("query", query)])
            .send()
            .await?;

        let bytes: Bytes = handle_res(res).await?;

        if log::log_enabled!(log::Level::Trace) {
            let raw = serde_json::from_slice::<serde_json::Value>(&bytes)?;
            log::trace!("search_by_name: {raw}");
        }

        let mut output = Vec::new();

        let data: Data<Vec<Row>> = serde_json::from_slice(&bytes)?;

        for row in data.results {
            let poster = match row.poster_path {
                Some(poster) => Some(Image::parse_tmdb(&poster)?),
                None => None,
            };

            let first_aired = match row.first_air_date {
                Some(first_aired) if !first_aired.is_empty() => Some(str::parse(&first_aired)?),
                _ => None,
            };

            output.push(SearchSeries {
                id: RemoteSeriesId::Tmdb { id: row.id },
                name: row.name,
                poster,
                overview: row.overview,
                first_aired,
            });
        }

        return Ok(output);

        #[derive(Deserialize)]
        struct Row {
            id: u32,
            name: String,
            #[serde(default)]
            overview: Option<String>,
            #[serde(default)]
            poster_path: Option<String>,
            #[serde(default)]
            first_air_date: Option<String>,
        }
    }

    /// Load image data.
    pub(crate) async fn get_image_data(&self, id: &TmdbImage) -> Result<Vec<u8>> {
        let mut url = self.state.image_url.clone();
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
    results: T,
}
