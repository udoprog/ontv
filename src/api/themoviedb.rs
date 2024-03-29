use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use chrono::{DateTime, NaiveDate, Utc};
use leaky_bucket::RateLimiter;
use relative_path::RelativePath;
use reqwest::header;
use reqwest::{Method, RequestBuilder, Response, StatusCode, Url};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_repr::Deserialize_repr;

use crate::api::common;
use crate::model::*;
use crate::service::{NewEpisode, UpdateMovie, UpdateSeries};

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
    limit: Arc<RateLimiter>,
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
            limit: Arc::new(
                RateLimiter::builder()
                    .max(50)
                    .initial(0)
                    .refill(1)
                    .interval(Duration::from_millis(100))
                    .build(),
            ),
        })
    }

    /// Set API key to the given value.
    pub(crate) fn set_api_key<S>(&mut self, api_key: &S)
    where
        S: ?Sized + AsRef<str>,
    {
        self.api_key = api_key.as_ref().into();
    }

    async fn request<I>(&self, method: Method, segments: I) -> RequestBuilder
    where
        I: IntoIterator,
        I::Item: AsRef<str>,
    {
        self.limit.acquire_one().await;

        let mut url = self.state.base_url.clone();

        if let Ok(mut m) = url.path_segments_mut() {
            m.extend(segments);
        }

        self.client
            .request(method, url)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
    }

    /// Request with (hopefully cached) authorization.
    async fn request_with_auth<I>(&self, method: Method, segments: I) -> RequestBuilder
    where
        I: IntoIterator,
        I::Item: AsRef<str>,
    {
        self.request(method, segments)
            .await
            .query(&[("api_key", self.api_key.as_ref())])
    }

    /// Search series result.
    pub(crate) async fn search_series(&self, query: &str) -> Result<Vec<SearchSeries>> {
        #[derive(Deserialize)]
        struct Row {
            id: u32,
            #[serde(default)]
            original_name: Option<String>,
            #[serde(default)]
            overview: Option<String>,
            #[serde(default)]
            poster_path: Option<String>,
            #[serde(default)]
            first_air_date: Option<String>,
        }

        let res = self
            .request_with_auth(Method::GET, &["search", "tv"])
            .await
            .query(&[&("query", query)])
            .send()
            .await?;

        let data: Data<Vec<Row>> = response("search/tv", res).await?;
        let mut output = Vec::with_capacity(data.results.len());

        for row in data.results {
            let first_aired = match row.first_air_date {
                Some(first_aired) if !first_aired.is_empty() => Some(str::parse(&first_aired)?),
                _ => None,
            };

            let poster = row.poster_path.as_deref().and_then(ImageV2::tmdb);

            output.push(SearchSeries {
                id: RemoteId::Tmdb { id: row.id },
                name: row.original_name.unwrap_or_default(),
                poster,
                overview: row.overview.unwrap_or_default(),
                first_aired,
            });
        }

        Ok(output)
    }

    /// Search movies result.
    pub(crate) async fn search_movies(&self, query: &str) -> Result<Vec<SearchMovie>> {
        #[derive(Deserialize)]
        struct Row {
            id: u32,
            #[serde(default)]
            original_title: Option<String>,
            #[serde(default)]
            #[allow(unused)]
            original_language: Option<String>,
            #[serde(default)]
            overview: Option<String>,
            #[serde(default)]
            poster_path: Option<String>,
            #[serde(default)]
            release_date: Option<String>,
        }

        let res = self
            .request_with_auth(Method::GET, &["search", "movie"])
            .await
            .query(&[&("query", query)])
            .send()
            .await?;

        let data: Data<Vec<Row>> = response("search/movie", res).await?;
        let mut output = Vec::with_capacity(data.results.len());

        for row in data.results {
            let poster = row.poster_path.as_deref().and_then(ImageV2::tmdb);

            let release_date = match row.release_date {
                Some(release_date) if !release_date.is_empty() => Some(str::parse(&release_date)?),
                _ => None,
            };

            output.push(SearchMovie {
                id: RemoteId::Tmdb { id: row.id },
                title: row.original_title.unwrap_or_default(),
                poster,
                overview: row.overview.unwrap_or_default(),
                release_date,
            });
        }

        Ok(output)
    }

    /// Download series information.
    pub(crate) async fn series(
        &self,
        id: u32,
        lookup: impl common::LookupSeriesId,
        if_none_match: Option<&Etag>,
    ) -> Result<
        Option<(
            UpdateSeries,
            BTreeSet<RemoteId>,
            Option<Etag>,
            Option<DateTime<Utc>>,
            Vec<Season>,
        )>,
    > {
        #[derive(Deserialize)]
        struct Details {
            id: u32,
            #[serde(default)]
            name: Option<String>,
            #[serde(default)]
            original_name: Option<String>,
            #[serde(default)]
            original_language: Option<String>,
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

        let mut details = self
            .request_with_auth(Method::GET, &["tv", &id.to_string()])
            .await;

        if let Some(etag) = if_none_match {
            details = details.header(header::IF_NONE_MATCH, etag.as_ref());
        }

        let details = details.send().await?;

        if details.status() == StatusCode::NOT_MODIFIED {
            return Ok(None);
        }

        let last_modified = common::parse_last_modified(&details)?;
        let last_etag = common::parse_etag(&details);

        let details = response::<Details, _>(format!("tv/{id}"), details).await?;

        let language = details.original_language.filter(|s| !s.is_empty());

        let external_ids = self
            .request_with_auth(Method::GET, &["tv", &id.to_string(), "external_ids"])
            .await
            .send()
            .await?;

        let languages;
        let images_query;

        let image_query = match language.as_deref() {
            Some(lang) => {
                languages = format!("{lang},null");
                images_query = [("include_image_language", &languages)];
                &images_query[..]
            }
            None => &[][..],
        };

        let images = self
            .request_with_auth(Method::GET, &["tv", &id.to_string(), "images"])
            .await
            .query(&image_query)
            .send()
            .await?;

        let (external_ids, images) = tokio::try_join!(
            response::<ExternalIds, _>(format!("tv/{id}/external_ids"), external_ids),
            response::<Images, _>(format!("tv/{id}/images"), images)
        )?;

        let remote_id = RemoteId::Tmdb { id: details.id };

        let mut remote_ids = BTreeSet::from([remote_id]);

        for remote_id in external_ids.as_remote_series() {
            remote_ids.insert(remote_id?);
        }

        // Try to lookup the series by known remote ids.
        let id = lookup
            .lookup(remote_ids.iter().copied())
            .unwrap_or_else(SeriesId::random);

        let mut graphics = SeriesGraphics::default();
        graphics.poster = details.poster_path.as_deref().and_then(ImageV2::tmdb);

        for image in images.posters {
            graphics.posters.extend(ImageV2::tmdb(&image.file_path));
        }

        graphics.banner = details.backdrop_path.as_deref().and_then(ImageV2::tmdb);

        for image in images.backdrops {
            graphics.banners.extend(ImageV2::tmdb(&image.file_path));
        }

        let series = UpdateSeries {
            id,
            title: details.original_name.or(details.name).unwrap_or_default(),
            language,
            first_air_date: details.first_air_date,
            overview: details.overview.unwrap_or_default(),
            graphics,
            remote_id,
        };

        let mut seasons = Vec::with_capacity(details.seasons.len());

        for s in details.seasons {
            let mut graphics = SeasonGraphics::default();
            graphics.poster = s.poster_path.as_deref().and_then(ImageV2::tmdb);

            seasons.push(Season {
                number: match s.season_number {
                    Some(n) if n > 0 => SeasonNumber::Number(n),
                    _ => SeasonNumber::Specials,
                },
                air_date: s.air_date,
                name: s.name,
                overview: s.overview.unwrap_or_default(),
                compat_poster: None,
                graphics,
            });
        }

        Ok(Some((
            series,
            remote_ids,
            last_etag,
            last_modified,
            seasons,
        )))
    }

    /// Download episodes.
    pub(crate) async fn download_episodes(
        &self,
        series_id: u32,
        season: SeasonNumber,
        language: Option<&str>,
        lookup: impl common::LookupEpisodeId,
    ) -> Result<Vec<NewEpisode>> {
        #[derive(Deserialize)]
        struct Details {
            #[serde(default)]
            episodes: Vec<EpisodeDetail>,
        }

        let season_number = match season {
            SeasonNumber::Specials => 0,
            SeasonNumber::Number(n) => n,
        };

        let pair;

        let query = match language {
            Some(language) => {
                pair = [("language", language)];
                &pair[..]
            }
            None => &[],
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
            .await
            .query(query)
            .send()
            .await?;

        let details: Details = response("tv/{id}/season/{number}", details).await?;

        let mut episodes = Vec::with_capacity(details.episodes.len());

        for e in details.episodes {
            let remote_id = RemoteEpisodeId::Tmdb { id: e.id };

            let d = self
                .download_episode_details(remote_id, series_id, season_number, e, &lookup)
                .await?;

            let mut graphics = EpisodeGraphics::default();
            graphics.filename = d.episode.still_path.as_deref().and_then(ImageV2::tmdb);

            let episode = Episode {
                id: d.id,
                name: d.episode.name,
                overview: d.episode.overview.unwrap_or_default(),
                absolute_number: None,
                season,
                number: d.episode.episode_number,
                aired: d.episode.air_date,
                compat_filename: None,
                graphics,
                remote_id: Some(d.remote_id),
            };

            episodes.push(NewEpisode {
                episode,
                remote_ids: d.remote_ids,
            });
        }

        Ok(episodes)
    }

    async fn download_episode_details(
        &self,
        remote_id: RemoteEpisodeId,
        series_id: u32,
        season_number: u32,
        episode: EpisodeDetail,
        lookup: &impl common::LookupEpisodeId,
    ) -> Result<DownloadEpisode> {
        tracing::trace!(
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
                tracing::warn!(
                    "missing external ids for: series: {series_id}, season: {season_number}, episode: {}",
                    episode.episode_number
                );

                ExternalIds::default()
            }
        };

        let mut remote_ids = BTreeSet::from([remote_id]);

        for remote_id in external_ids.as_remote_episodes() {
            remote_ids.insert(remote_id?);
        }

        let id = match id {
            Some(id) => id,
            None => lookup
                .lookup(remote_ids.iter().copied())
                .unwrap_or_else(EpisodeId::random),
        };

        Ok(DownloadEpisode {
            remote_ids,
            remote_id,
            id,
            episode,
        })
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

        let external_ids = self
            .request_with_auth(Method::GET, &path)
            .await
            .send()
            .await?;

        if external_ids.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }

        let external_ids = response::<ExternalIds, _>(
            format!("tv/{season_id}/season/{season_number}/episode/{episode_number}/external_ids"),
            external_ids,
        )
        .await?;
        Ok(Some(external_ids))
    }

    /// Download movie information.
    pub(crate) async fn movie(
        &self,
        id: u32,
        lookup: impl common::LookupMovieId,
        if_none_match: Option<&Etag>,
    ) -> Result<
        Option<(
            UpdateMovie,
            BTreeSet<RemoteId>,
            Option<Etag>,
            Option<DateTime<Utc>>,
        )>,
    > {
        #[derive(Deserialize)]
        struct Details {
            id: u32,
            #[serde(default)]
            title: Option<String>,
            #[serde(default)]
            original_title: Option<String>,
            #[serde(default)]
            original_language: Option<String>,
            #[serde(default)]
            overview: Option<String>,
            #[serde(default)]
            poster_path: Option<String>,
            #[serde(default)]
            backdrop_path: Option<String>,
            #[serde(default)]
            release_date: Option<NaiveDate>,
        }

        let mut details = self
            .request_with_auth(Method::GET, &["movie", &id.to_string()])
            .await;

        if let Some(etag) = if_none_match {
            details = details.header(header::IF_NONE_MATCH, etag.as_ref());
        }

        let details = details.send().await?;

        if details.status() == StatusCode::NOT_MODIFIED {
            return Ok(None);
        }

        let last_modified = common::parse_last_modified(&details)?;
        let last_etag = common::parse_etag(&details);

        let details = response::<Details, _>(format!("movie/{id}"), details).await?;

        let language = details.original_language.filter(|s| !s.is_empty());

        let release_dates = self
            .request_with_auth(Method::GET, &["movie", &id.to_string(), "release_dates"])
            .await;

        let external_ids = self
            .request_with_auth(Method::GET, &["movie", &id.to_string(), "external_ids"])
            .await;

        let (release_dates, external_ids) =
            tokio::try_join!(release_dates.send(), external_ids.send())?;

        let languages;
        let images_query;

        let image_query = match language.as_deref() {
            Some(lang) => {
                languages = format!("{lang},null");
                images_query = [("include_image_language", &languages)];
                &images_query[..]
            }
            None => &[][..],
        };

        let images = self
            .request_with_auth(Method::GET, &["movie", &id.to_string(), "images"])
            .await
            .query(&image_query)
            .send()
            .await?;

        let (release_dates, external_ids, images) = tokio::try_join!(
            response::<ReleaseDates, _>(format!("movie/{id}/release_dates"), release_dates),
            response::<ExternalIds, _>(format!("movie/{id}/external_ids"), external_ids),
            response::<Images, _>(format!("movie/{id}/images"), images)
        )?;

        tracing::trace!(?release_dates);

        let remote_id = RemoteId::Tmdb { id: details.id };

        let mut remote_ids = BTreeSet::from([remote_id]);

        for remote_id in external_ids.as_remote_movie() {
            remote_ids.insert(remote_id?);
        }

        // Try to lookup the series by known remote ids.
        let id = lookup
            .lookup(remote_ids.iter().copied())
            .unwrap_or_else(MovieId::random);

        let mut graphics = MovieGraphics::default();
        graphics.poster = details.poster_path.as_deref().and_then(ImageV2::tmdb);

        for image in images.posters {
            graphics.posters.extend(ImageV2::tmdb(&image.file_path));
        }

        graphics.banner = details.backdrop_path.as_deref().and_then(ImageV2::tmdb);

        for image in images.backdrops {
            graphics.banners.extend(ImageV2::tmdb(&image.file_path));
        }

        let release_dates = {
            let mut out = Vec::new();

            for result in release_dates.results {
                let mut release_dates = Vec::with_capacity(result.release_dates.len());

                for release_date in result.release_dates {
                    release_dates.push(MovieReleaseDate {
                        date: release_date.release_date,
                        kind: match release_date.ty_ {
                            ReleaseType::Premiere => MovieReleaseKind::Premiere,
                            ReleaseType::TheatricalLimited => MovieReleaseKind::TheatricalLimited,
                            ReleaseType::Theatrical => MovieReleaseKind::Theatrical,
                            ReleaseType::Digital => MovieReleaseKind::Digital,
                            ReleaseType::Physical => MovieReleaseKind::Physical,
                            ReleaseType::Tv => MovieReleaseKind::Tv,
                        },
                    });
                }

                out.push(MovieReleaseDates {
                    country: result.iso_3166_1,
                    dates: release_dates,
                });
            }

            out
        };

        let series = UpdateMovie {
            id,
            title: details.original_title.or(details.title).unwrap_or_default(),
            language,
            release_date: details.release_date,
            overview: details.overview.unwrap_or_default(),
            graphics,
            remote_id,
            release_dates,
        };

        Ok(Some((series, remote_ids, last_etag, last_modified)))
    }

    /// Load image data from path.
    pub(crate) async fn download_image_path(&self, path: &RelativePath) -> Result<Vec<u8>> {
        let mut url = self.state.image_url.clone();

        if let Ok(mut segments) = url.path_segments_mut() {
            segments.extend(["t", "p", "original"]);

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

/// Converting a response from JSON.
async fn response<T, W>(what: W, res: Response) -> Result<T>
where
    W: fmt::Display + Send,
    T: DeserializeOwned,
{
    async fn inner<T, W>(what: &W, res: Response) -> Result<T>
    where
        W: fmt::Display + Send,
        T: DeserializeOwned,
    {
        if !res.status().is_success() {
            bail!("{}: {}", res.status(), res.text().await?);
        }

        let output = res.bytes().await?;

        if tracing::enabled!(tracing::Level::TRACE) {
            let text = String::from_utf8_lossy(&output);
            tracing::trace!("{what}: {text}");
        }

        Ok(serde_json::from_slice(&output)?)
    }

    inner(&what, res).await.with_context(|| anyhow!("{what}"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize_repr)]
#[repr(u8)]
enum ReleaseType {
    Premiere = 1,
    TheatricalLimited = 2,
    Theatrical = 3,
    Digital = 4,
    Physical = 5,
    Tv = 6,
}

#[derive(Debug, Deserialize)]
struct ReleaseDate {
    #[serde(default)]
    #[allow(unused)]
    certification: String,
    #[allow(unused)]
    descriptors: Vec<serde_json::Value>,
    #[serde(default)]
    #[allow(unused)]
    iso_639_1: String,
    #[allow(unused)]
    note: String,
    release_date: DateTime<Utc>,
    #[serde(rename = "type")]
    ty_: ReleaseType,
}

#[derive(Debug, Deserialize)]
struct ReleaseDateResult {
    #[serde(default)]
    iso_3166_1: String,
    #[serde(default)]
    release_dates: Vec<ReleaseDate>,
}

#[derive(Debug, Deserialize)]
struct ReleaseDates {
    #[allow(unused)]
    id: u32,
    results: Vec<ReleaseDateResult>,
}

#[derive(Default, Deserialize)]
struct ExternalIds {
    imdb_id: Option<String>,
    tvdb_id: Option<u32>,
}

impl ExternalIds {
    /// Coerce into remote series ids.
    pub(crate) fn as_remote_series(&self) -> impl Iterator<Item = Result<RemoteId>> {
        let a = self.tvdb_id.map(|id| Ok(RemoteId::Tvdb { id }));

        let b = self.imdb_id.as_ref().and_then(|id| {
            if id.is_empty() {
                return None;
            }

            let id = match Raw::new(&id).context("imdb id overflow") {
                Ok(id) => id,
                Err(e) => return Some(Err(e)),
            };

            Some(Ok(RemoteId::Imdb { id }))
        });

        a.into_iter().chain(b)
    }

    /// Coerce into remote movie ids.
    pub(crate) fn as_remote_movie(&self) -> impl Iterator<Item = Result<RemoteId>> {
        let a = self.imdb_id.as_ref().and_then(|id| {
            if id.is_empty() {
                return None;
            }

            let id = match Raw::new(&id).context("imdb id overflow") {
                Ok(id) => id,
                Err(e) => return Some(Err(e)),
            };

            Some(Ok(RemoteId::Imdb { id }))
        });

        a.into_iter()
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
struct Image {
    file_path: String,
}

#[derive(Deserialize)]
struct Images {
    #[serde(default)]
    backdrops: Vec<Image>,
    #[serde(default)]
    posters: Vec<Image>,
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

struct DownloadEpisode {
    remote_ids: BTreeSet<RemoteEpisodeId>,
    remote_id: RemoteEpisodeId,
    id: EpisodeId,
    episode: EpisodeDetail,
}
