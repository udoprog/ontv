use anyhow::Result;
use chrono::{DateTime, Utc};
use reqwest::{header, Response};

use crate::model::{EpisodeId, Etag, MovieId, RemoteEpisodeId, RemoteId, SeriesId};

/// Parse out last modified header if present.
pub(crate) fn parse_last_modified(res: &Response) -> Result<Option<DateTime<Utc>>> {
    let Some(last_modified) = res.headers().get(header::LAST_MODIFIED) else {
        return Ok(None);
    };

    let last_modified = DateTime::parse_from_rfc2822(last_modified.to_str()?)?;
    let last_modified = last_modified.naive_utc();
    Ok(Some(DateTime::from_naive_utc_and_offset(
        last_modified,
        Utc,
    )))
}

/// Parse out etag if available.
pub(crate) fn parse_etag(response: &Response) -> Option<Etag> {
    let header = response.headers().get(header::ETAG)?;
    Some(Etag::new(header.as_bytes()))
}

/// Helper trait to lookup a series id.
pub(crate) trait LookupSeriesId {
    fn lookup<I>(&self, ids: I) -> Option<SeriesId>
    where
        I: IntoIterator<Item = RemoteId>;
}

/// Helper trait to lookup a series id.
pub(crate) trait LookupMovieId {
    fn lookup<I>(&self, ids: I) -> Option<MovieId>
    where
        I: IntoIterator<Item = RemoteId>;
}

/// Helper trait to lookup an episode id.
pub(crate) trait LookupEpisodeId {
    fn lookup<I>(&self, ids: I) -> Option<EpisodeId>
    where
        I: IntoIterator<Item = RemoteEpisodeId>;
}

impl<F> LookupSeriesId for F
where
    F: Fn(RemoteId) -> Option<SeriesId>,
{
    #[inline]
    fn lookup<I>(&self, ids: I) -> Option<SeriesId>
    where
        I: IntoIterator<Item = RemoteId>,
    {
        for id in ids {
            if let Some(id) = (self)(id) {
                return Some(id);
            }
        }

        None
    }
}

impl<F> LookupMovieId for F
where
    F: Fn(RemoteId) -> Option<MovieId>,
{
    #[inline]
    fn lookup<I>(&self, ids: I) -> Option<MovieId>
    where
        I: IntoIterator<Item = RemoteId>,
    {
        for id in ids {
            if let Some(id) = (self)(id) {
                return Some(id);
            }
        }

        None
    }
}

impl<F> LookupEpisodeId for F
where
    F: Fn(RemoteEpisodeId) -> Option<EpisodeId>,
{
    #[inline]
    fn lookup<I>(&self, ids: I) -> Option<EpisodeId>
    where
        I: IntoIterator<Item = RemoteEpisodeId>,
    {
        for id in ids {
            if let Some(id) = (self)(id) {
                return Some(id);
            }
        }

        None
    }
}
