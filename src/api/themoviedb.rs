use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use bytes::Bytes;
use chrono::NaiveDate;
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use reqwest::header;
use reqwest::StatusCode;
use reqwest::{Method, RequestBuilder, Response, Url};
use serde::Deserialize;

use crate::api::common;
use crate::model::{
    Episode, EpisodeId, Etag, Image, Raw, RemoteEpisodeId, RemoteMovieId, RemoteSeriesId,
    SearchMovie, SearchSeries, Season, SeasonNumber, Series, SeriesId, TmdbImage,
};
use crate::service::NewEpisode;

const BASE_URL: &str = "https://api.themoviedb.org/3";
const IMAGE_URL: &str = "https://image.tmdb.org";
const IDLE_TIMEOUT: Duration = Duration::from_secs(10);
const PARALLELISM: usize = 10;

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
            let poster = process_image(row.poster_path.as_deref()).context("bad poster image")?;

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

    /// Search movies result.
    pub(crate) async fn search_movies(&self, query: &str) -> Result<Vec<SearchMovie>> {
        let res = self
            .request_with_auth(Method::GET, &["search", "movie"])
            .query(&[&("query", query)])
            .send()
            .await?;

        let bytes: Bytes = handle_res(res).await?;

        if log::log_enabled!(log::Level::Trace) {
            let raw = serde_json::from_slice::<serde_json::Value>(&bytes)?;
            log::trace!("search/movie: {raw}");
        }

        let mut output = Vec::new();

        let data: Data<Vec<Row>> = serde_json::from_slice(&bytes)?;

        for row in data.results {
            let poster = process_image(row.poster_path.as_deref()).context("bad poster image")?;

            let release_date = match row.release_date {
                Some(release_date) if !release_date.is_empty() => Some(str::parse(&release_date)?),
                _ => None,
            };

            output.push(SearchMovie {
                id: RemoteMovieId::Tmdb { id: row.id },
                title: row.original_title,
                poster,
                overview: row.overview,
                release_date,
            });
        }

        return Ok(output);

        #[derive(Deserialize)]
        struct Row {
            id: u32,
            original_title: String,
            #[serde(default)]
            overview: Option<String>,
            #[serde(default)]
            poster_path: Option<String>,
            #[serde(default)]
            release_date: Option<String>,
        }
    }

    /// Load image data.
    pub(crate) async fn download_image(&self, id: &TmdbImage) -> Result<Vec<u8>> {
        let mut url = self.state.image_url.clone();
        url.set_path(&id.to_string());
        let res = self.client.get(url).send().await?;
        Ok(res.bytes().await?.to_vec())
    }

    /// Download series information.
    pub(crate) async fn series(
        &self,
        id: u32,
        lookup: impl common::LookupSeriesId,
        if_none_match: Option<&Etag>,
    ) -> Result<Option<(Series, BTreeSet<RemoteSeriesId>, Option<Etag>, Vec<Season>)>> {
        let mut details = self.request_with_auth(Method::GET, &["tv", &id.to_string()]);

        if let Some(etag) = if_none_match {
            details = details.header(header::IF_NONE_MATCH, etag.as_ref());
        }

        let details = details.send().await?;

        if details.status() == StatusCode::NOT_MODIFIED {
            return Ok(None);
        }

        let external_ids = self
            .request_with_auth(Method::GET, &["tv", &id.to_string(), "external_ids"])
            .send()
            .await?;

        let last_modified = common::parse_last_modified(&details)?;
        let last_etag = common::parse_etag(&details);

        let (external_ids, details) =
            tokio::try_join!(handle_res(external_ids), handle_res(details))?;

        if log::log_enabled!(log::Level::Trace) {
            let details = serde_json::from_slice::<serde_json::Value>(&details)?;
            log::trace!("details: {details}");
            let external_ids = serde_json::from_slice::<serde_json::Value>(&external_ids)?;
            log::trace!("external_ids: {external_ids}");
        }

        let details: Details = serde_json::from_slice(&details).context("details response")?;
        let external_ids: ExternalIds =
            serde_json::from_slice(&external_ids).context("remote ids")?;

        let remote_id = RemoteSeriesId::Tmdb { id: details.id };

        let mut remote_ids = BTreeSet::from([remote_id]);

        for remote_id in external_ids.as_remote_series() {
            remote_ids.insert(remote_id?);
        }

        // Try to lookup the series by known remote ids.
        let id = lookup
            .lookup(remote_ids.iter().copied())
            .unwrap_or_else(SeriesId::random);

        let poster = process_image(details.poster_path.as_deref()).context("poster image")?;
        let banner = process_image(details.backdrop_path.as_deref()).context("backdrop image")?;

        let series = Series {
            id,
            title: details.name.unwrap_or_default(),
            first_air_date: details.first_air_date,
            overview: details.overview,
            poster,
            banner,
            fanart: None,
            tracked: true,
            last_modified,
            compat_last_etag: None,
            compat_last_sync: BTreeMap::new(),
            remote_id: Some(remote_id),
        };

        let mut seasons = Vec::with_capacity(details.seasons.len());

        for s in details.seasons {
            let poster = process_image(s.poster_path.as_deref()).context("season poster image")?;

            seasons.push(Season {
                number: match s.season_number {
                    Some(n) if n > 0 => SeasonNumber::Number(n),
                    _ => SeasonNumber::Specials,
                },
                air_date: s.air_date,
                name: s.name,
                overview: s.overview,
                poster,
            });
        }

        return Ok(Some((series, remote_ids, last_etag, seasons)));

        #[derive(Deserialize)]
        struct Details {
            id: u32,
            #[serde(default)]
            name: Option<String>,
            #[serde(default)]
            overview: Option<String>,
            #[serde(default)]
            poster_path: Option<String>,
            #[serde(default)]
            backdrop_path: Option<String>,
            #[serde(default)]
            first_air_date: Option<NaiveDate>,
            #[serde(default)]
            seasons: Vec<SeasonDetails>,
        }

        #[derive(Deserialize)]
        struct SeasonDetails {
            season_number: Option<u32>,
            #[serde(default)]
            air_date: Option<NaiveDate>,
            #[serde(default)]
            name: Option<String>,
            #[serde(default)]
            overview: Option<String>,
            #[serde(default)]
            poster_path: Option<String>,
        }
    }

    /// Download episodes.
    pub(crate) async fn episodes(
        &self,
        series_id: u32,
        season: SeasonNumber,
        lookup: impl common::LookupEpisodeId,
    ) -> Result<Vec<NewEpisode>> {
        let season_number = match season {
            SeasonNumber::Specials => 0,
            SeasonNumber::Number(n) => n,
        };

        let details = self
            .request_with_auth(
                Method::GET,
                &[
                    "tv",
                    &series_id.to_string(),
                    "season",
                    &season_number.to_string(),
                ],
            )
            .send()
            .await?;

        let details = handle_res(details).await?;

        if log::log_enabled!(log::Level::Trace) {
            let details = serde_json::from_slice::<serde_json::Value>(&details)?;
            log::trace!("details: {details}");
        }

        let details: Details = serde_json::from_slice(&details).context("details response")?;

        let mut episodes = Vec::new();
        let mut output = Vec::with_capacity(details.episodes.len());
        let mut it = details.episodes.into_iter();

        let mut tasks = FuturesUnordered::new();

        let mut n = 0;

        loop {
            while tasks.len() < PARALLELISM {
                let Some(e) = it.next() else {
                    break;
                };

                let remote_id = RemoteEpisodeId::Tmdb { id: e.id };

                tasks.push(self.download_remote_ids(
                    n,
                    remote_id,
                    series_id,
                    season_number,
                    e,
                    &lookup,
                ));

                n += 1;
            }

            let Some(result) = tasks.next().await else {
                break;
            };

            output.push(result?);
        }

        output.sort_by(|a, b| a.0.cmp(&b.0));

        for (_, remote_ids, remote_id, id, e) in output {
            let filename = process_image(e.still_path.as_deref()).context("bad still image")?;

            let episode = Episode {
                id,
                name: e.name,
                overview: e.overview,
                absolute_number: None,
                season,
                number: e.episode_number,
                aired: e.air_date,
                filename,
                remote_id: Some(remote_id),
            };

            episodes.push(NewEpisode {
                episode,
                remote_ids,
            });
        }

        return Ok(episodes);

        #[derive(Deserialize)]
        struct Details {
            #[serde(default)]
            episodes: Vec<EpisodeDetail>,
        }
    }

    async fn download_remote_ids(
        &self,
        index: usize,
        remote_id: RemoteEpisodeId,
        series_id: u32,
        season_number: u32,
        episode: EpisodeDetail,
        lookup: &impl common::LookupEpisodeId,
    ) -> Result<(
        usize,
        BTreeSet<RemoteEpisodeId>,
        RemoteEpisodeId,
        EpisodeId,
        EpisodeDetail,
    )> {
        log::trace!(
            "downloading remote ids for: series: {series_id}, season: {season_number}, episode: {}",
            episode.episode_number
        );

        let id = lookup.lookup([remote_id]);

        let external_ids = self
            .episode_external_ids(series_id, season_number, episode.episode_number)
            .await?;

        let external_ids = match external_ids {
            Some(external_ids) => external_ids,
            None => {
                log::warn!(
                    "missing external ids for: series: {series_id}, season: {season_number}, episode: {}",
                    episode.episode_number
                );

                ExternalIds::default()
            }
        };

        let mut remotes = BTreeSet::from([remote_id]);

        for remote_id in external_ids.as_remote_episodes() {
            remotes.insert(remote_id?);
        }

        let id = match id {
            Some(id) => id,
            None => lookup
                .lookup(remotes.iter().copied())
                .unwrap_or_else(EpisodeId::random),
        };

        Ok((index, remotes, remote_id, id, episode))
    }

    /// Get external IDs for an episode.
    async fn episode_external_ids(
        &self,
        season_id: u32,
        season_number: u32,
        episode_number: u32,
    ) -> Result<Option<ExternalIds>> {
        let path = [
            "tv",
            &season_id.to_string(),
            "season",
            &season_number.to_string(),
            "episode",
            &episode_number.to_string(),
            "external_ids",
        ];

        let external_ids = self.request_with_auth(Method::GET, &path).send().await?;

        if external_ids.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }

        let external_ids = handle_res(external_ids).await?;
        let external_ids: ExternalIds = serde_json::from_slice(&external_ids)?;
        Ok(Some(external_ids))
    }
}

/// Handle converting response to JSON.
async fn handle_res(res: Response) -> Result<Bytes> {
    if !res.status().is_success() {
        bail!("{}: {}", res.status(), res.text().await?);
    }

    Ok(res.bytes().await?)
}

/// Process an optional image.
fn process_image(image: Option<&str>) -> Result<Option<Image>> {
    match image {
        Some(image) if !image.is_empty() => Ok(Some(Image::parse_tmdb(image)?)),
        _ => Ok(None),
    }
}

#[derive(Default, Deserialize)]
struct ExternalIds {
    imdb_id: Option<String>,
    tvdb_id: Option<u32>,
}

impl ExternalIds {
    /// Coerce into remote series ids.
    pub(crate) fn as_remote_series(&self) -> impl Iterator<Item = Result<RemoteSeriesId>> {
        let a = self.tvdb_id.map(|id| Ok(RemoteSeriesId::Tvdb { id }));

        let b = self.imdb_id.as_ref().and_then(|id| {
            if id.is_empty() {
                return None;
            }

            let id = match Raw::new(&id).context("imdb id overflow") {
                Ok(id) => id,
                Err(e) => return Some(Err(e)),
            };

            Some(Ok(RemoteSeriesId::Imdb { id }))
        });

        a.into_iter().chain(b)
    }

    /// Coerce into remote episode ids.
    pub(crate) fn as_remote_episodes(&self) -> impl Iterator<Item = Result<RemoteEpisodeId>> {
        let a = self.tvdb_id.map(|id| Ok(RemoteEpisodeId::Tvdb { id }));

        let b = self.imdb_id.as_ref().and_then(|id| {
            if id.is_empty() {
                return None;
            }

            let id = match Raw::new(&id).context("imdb id overflow") {
                Ok(id) => id,
                Err(e) => return Some(Err(e)),
            };

            Some(Ok(RemoteEpisodeId::Imdb { id }))
        });

        a.into_iter().chain(b)
    }
}

#[derive(Deserialize)]
struct Data<T> {
    results: T,
}

#[derive(Deserialize)]
struct EpisodeDetail {
    id: u32,
    episode_number: u32,
    #[serde(default)]
    air_date: Option<NaiveDate>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    overview: Option<String>,
    #[serde(default)]
    still_path: Option<String>,
}
