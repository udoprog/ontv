use std::path::Path;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::runtime;

use crate::model::{Raw, RemoteSeriesId, SeasonNumber, SeriesId};
use crate::search::Tokens;
use crate::service::Service;

/// Import trakt watched history from the given path.
pub fn import_trakt_watched(
    service: &mut Service,
    path: &Path,
    filter: Option<&str>,
    remove: bool,
    import_missing: bool,
) -> Result<()> {
    let runtime = runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    let filter = filter.map(Tokens::new);

    use std::fs::File;
    let f = File::open(path)?;
    let rows: Vec<serde_json::Value> = serde_json::from_reader(f)?;

    for (index, row) in rows.into_iter().enumerate() {
        let entry: Entry = serde_json::from_value(row.clone())?;

        if let Some(filter) = &filter {
            if !filter.matches(&entry.show.title) {
                continue;
            }
        }

        tracing::trace!("{index}: {row}");

        let mut ids = Vec::new();

        let tmdb_remote_id = RemoteSeriesId::Tmdb {
            id: entry.show.ids.tmdb,
        };

        ids.push(RemoteSeriesId::Tvdb {
            id: entry.show.ids.tvdb,
        });
        ids.push(tmdb_remote_id);
        ids.push(RemoteSeriesId::Imdb {
            id: Raw::new(&entry.show.ids.imdb).context("imdb id")?,
        });

        let now = Utc::now();

        // TODO: use more databases.
        let series_id = match service.existing_by_remote_ids(ids) {
            Some(series_id) => {
                if service.series(&series_id).is_none() && import_missing {
                    let Some(..) = runtime.block_on(download_series(service, &now, &entry, &tmdb_remote_id))? else {
                        continue;
                    };
                }

                series_id
            }
            None => {
                if !import_missing {
                    tracing::warn!(
                        "show `{}` is not a local series and not configured to import missing",
                        entry.show.title
                    );
                    continue;
                };

                let Some(id) = runtime.block_on(download_series(service, &now, &entry, &tmdb_remote_id))? else {
                    continue;
                };

                id
            }
        };

        tracing::trace!("{index}: {series_id}: {entry:?}");

        if remove {
            service.clear_watches(&series_id);
        }

        let mut any = false;

        for season in &entry.seasons {
            for import in &season.episodes {
                let Some(episode) = service.find_episode_by(&series_id, |e| e.season == SeasonNumber::Number(season.number) && e.number == import.number) else {
                    continue;
                };

                if service.watched(&episode.id).next().is_some() {
                    continue;
                }

                any = true;
                tracing::trace!("{index}: watch: {}", episode.id);
                service.insert_new_watch(series_id, episode.id, import.last_watched_at);
            }
        }

        if any {
            tracing::info!("imported watch history for `{}`", entry.show.title);
        }

        service.populate_pending(&now, &series_id);
        runtime.block_on(service.save_changes())?;
    }

    runtime.shutdown_background();
    Ok(())
}

async fn download_series(
    service: &mut Service,
    now: &DateTime<Utc>,
    entry: &Entry,
    remote_id: &RemoteSeriesId,
) -> Result<Option<SeriesId>> {
    tracing::info!("downloading `{}`", entry.show.title);

    let new_series = match service.download_series(remote_id, None, None).await {
        Ok(Some(new_series)) => new_series,
        Ok(None) => {
            anyhow::bail!("empty response")
        }
        Err(error) => {
            tracing::error!("failed to download `{}`: {error}", entry.show.title);
            return Ok(None);
        }
    };

    let series_id = new_series.series.id;
    service.insert_series(now, new_series);
    service.save_changes().await?;
    Ok(Some(series_id))
}

#[derive(Debug, Deserialize, Serialize)]
struct Episode {
    last_watched_at: DateTime<Utc>,
    number: u32,
}

#[derive(Debug, Deserialize, Serialize)]
struct Season {
    number: u32,
    episodes: Vec<Episode>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Ids {
    imdb: String,
    slug: String,
    tmdb: u32,
    trakt: u32,
    tvdb: u32,
    #[serde(default)]
    tvrage: Option<u32>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Show {
    title: String,
    ids: Ids,
}

#[derive(Debug, Deserialize, Serialize)]
struct Entry {
    show: Show,
    seasons: Vec<Season>,
}
