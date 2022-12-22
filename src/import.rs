use std::path::Path;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::runtime;

use crate::model::{Raw, RemoteSeriesId, SeasonNumber};
use crate::search::Tokens;
use crate::service::Service;

/// Import trakt watched history from the given path.
pub(crate) fn import_trakt_watched(
    service: &mut Service,
    path: &Path,
    filter: Option<&str>,
    remove: bool,
    import_missing: bool,
) -> Result<()> {
    let runtime = runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    let filter = filter.map(|filter| Tokens::new(filter));

    use std::fs::File;
    let f = File::open(path)?;
    let rows: Vec<serde_json::Value> = serde_json::from_reader(f)?;

    let now = Utc::now();

    for (index, row) in rows.into_iter().enumerate() {
        let entry: Entry = serde_json::from_value(row.clone())?;

        if let Some(filter) = &filter {
            if !filter.matches(&entry.show.title) {
                continue;
            }
        }

        log::trace!("{index}: {row}");

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

        // TODO: use more databases.
        let series_id = match service.existing_by_remote_ids(ids) {
            Some(series_id) => series_id,
            None => {
                if !import_missing {
                    log::warn!(
                        "show `{}` is not a local series and not configured to import missing",
                        entry.show.title
                    );
                    continue;
                };

                log::info!("downloading `{}`", entry.show.title);

                let (series_id, remote_id, new_series) =
                    runtime.block_on(service.download_series_by_remote(tmdb_remote_id));
                service.download_complete(series_id, remote_id);

                let new_series = match new_series {
                    Ok(new_series) => new_series,
                    Err(error) => {
                        log::error!("failed to download `{}`: {error}", entry.show.title);
                        continue;
                    }
                };

                let series_id = new_series.series_id();
                service.insert_new_series(new_series);
                runtime.block_on(service.save_changes())?;
                series_id
            }
        };

        log::trace!("{index}: {series_id}: {entry:?}");

        if remove {
            service.clear_watches(series_id);
        }

        let mut any = false;

        for season in &entry.seasons {
            for import in &season.episodes {
                let Some(episode) = service.find_episode_by(series_id, |e| e.season == SeasonNumber::Number(season.number) && e.number == import.number) else {
                    continue;
                };

                if !service.watched(episode.id).is_empty() {
                    continue;
                }

                any = true;
                log::trace!("{index}: watch: {}", episode.id);
                service.insert_new_watch(series_id, episode.id, import.last_watched_at);
            }
        }

        if any {
            log::info!("imported watch history for `{}`", entry.show.title);
        }

        service.populate_pending(&now, series_id, None);
        runtime.block_on(service.save_changes())?;
    }

    runtime.shutdown_background();
    Ok(())
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