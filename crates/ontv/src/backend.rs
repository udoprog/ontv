pub(crate) mod paths;

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, bail, Context, Error, Result};
use api::ImageHint;
use api::{ImageV2, SeasonNumber};
use futures::stream::FuturesUnordered;
use jiff::civil::Date;
use jiff::tz::TimeZone;
use jiff::ToSpan;
use jiff::{Timestamp, Zoned};
use tracing_futures::Instrument;

use crate::api::themoviedb;
use crate::api::thetvdb;
use crate::assets::ImageKey;
use crate::cache::{self};
use crate::files::{Change, EpisodeRef, Files, SeasonRef};
use crate::model::*;
use crate::queue::{CompletedTask, Task, TaskKind, TaskRef, TaskStatus};

// Cache series updates for 12 hours.
const CACHE_TIME: i64 = 3600 * 12;

/// A movie update as produced by an API.
#[derive(Debug, Clone)]
pub(crate) struct UpdateMovie {
    pub(crate) id: MovieId,
    pub(crate) title: String,
    #[allow(unused)]
    pub(crate) language: Option<String>,
    pub(crate) release_date: Option<Date>,
    pub(crate) overview: String,
    pub(crate) graphics: MovieGraphics,
    pub(crate) remote_id: RemoteId,
    pub(crate) release_dates: Vec<MovieReleaseDates>,
}

/// A series update as produced by an API.
#[derive(Debug, Clone)]
pub(crate) struct UpdateSeries {
    pub(crate) id: SeriesId,
    pub(crate) title: String,
    pub(crate) language: Option<String>,
    pub(crate) first_air_date: Option<Date>,
    pub(crate) overview: String,
    pub(crate) graphics: SeriesGraphics,
    pub(crate) remote_id: RemoteId,
}

/// New episode.
#[derive(Debug, Clone)]
pub(crate) struct NewEpisode {
    pub(crate) episode: Episode,
    pub(crate) remote_ids: BTreeSet<RemoteEpisodeId>,
}

/// Data encapsulating a newly added series.
#[derive(Debug, Clone)]
pub(crate) struct NewSeries {
    pub(crate) series: UpdateSeries,
    pub(crate) remote_ids: BTreeSet<RemoteId>,
    pub(crate) last_etag: Option<Etag>,
    pub(crate) last_modified: Option<Timestamp>,
    pub(crate) episodes: Vec<NewEpisode>,
    pub(crate) seasons: Vec<Season>,
}

/// Data encapsulating a newly added movie.
#[derive(Debug, Clone)]
pub(crate) struct NewMovie {
    pub(crate) movie: UpdateMovie,
    pub(crate) remote_ids: BTreeSet<RemoteId>,
    pub(crate) last_etag: Option<Etag>,
    pub(crate) last_modified: Option<Timestamp>,
}

/// A pending thing to watch.
#[derive(Debug, Clone, Copy)]
pub(crate) enum PendingRef<'a> {
    Episode {
        series: &'a Series,
        season: Option<SeasonRef<'a>>,
        episode: EpisodeRef<'a>,
    },
    Movie {
        movie: &'a Movie,
    },
}

impl<'a> PendingRef<'a> {
    /// Get the date at which the pending item is airs or is released.
    pub(crate) fn date(&self) -> Option<Date> {
        match self {
            PendingRef::Episode { episode, .. } => episode.aired,
            PendingRef::Movie { movie } => Some(
                movie
                    .earliest_release_date()?
                    .to_zoned(TimeZone::UTC)
                    .date(),
            ),
        }
    }

    /// Get poster for the given pending reference.
    pub(crate) fn poster(&self) -> Option<&'a ImageV2> {
        match self {
            PendingRef::Episode { series, season, .. } => {
                if let Some(season) = season.map(|s| s.into_season()) {
                    if let Some(image) = season.poster() {
                        return Some(image);
                    }
                }

                series.poster()
            }
            PendingRef::Movie { movie } => movie.poster(),
        }
    }

    /// Test if episode will air in the future.
    pub(crate) fn will_air(&self, today: &Date) -> bool {
        match self {
            PendingRef::Episode { episode, .. } => episode.will_air(today),
            PendingRef::Movie { movie } => movie.will_release(today),
        }
    }

    /// Test if pending ref has aired.
    pub(crate) fn has_aired(&self, today: &Date) -> bool {
        match self {
            PendingRef::Episode { episode, .. } => episode.has_aired(today),
            PendingRef::Movie { movie } => movie.has_released(today),
        }
    }
}

/// Background service taking care of all state handling.
pub struct Backend {
    paths: Arc<paths::Paths>,
    db: Files,
    tvdb: thetvdb::Client,
    tmdb: themoviedb::Client,
    do_not_save: bool,
    schedule: Vec<ScheduledDay>,
    now: Timestamp,
}

impl Backend {
    /// Construct and setup in-memory state of
    pub fn new(config: &Path, cache: &Path) -> Result<Self> {
        let paths = paths::Paths::new(config, cache);

        if !paths.images.is_dir() {
            tracing::debug!("Creating images directory: {}", paths.images.display());
            std::fs::create_dir_all(&paths.images)?;
        }

        let db = Files::load(&paths)?;
        let tvdb = thetvdb::Client::new(db.config.tvdb_legacy_apikey.as_str())?;
        let tmdb = themoviedb::Client::new(db.config.tmdb_api_key.as_str())?;

        let now = Timestamp::now();

        let mut this = Self {
            paths: Arc::new(paths),
            db,
            tvdb,
            tmdb,
            do_not_save: false,
            schedule: Vec::new(),
            now,
        };

        this.rebuild_schedule();
        Ok(this)
    }

    /// Current timestamp.
    pub(crate) fn now(&self) -> Timestamp {
        self.now
    }

    /// A scheduled day.
    pub(crate) fn schedule(&self) -> &[ScheduledDay] {
        &self.schedule
    }

    /// Get a single series.
    pub(crate) fn series(&self, id: &SeriesId) -> Option<&Series> {
        self.db.series.get(id)
    }

    /// Get a single movie.
    pub(crate) fn movie(&self, id: &MovieId) -> Option<&Movie> {
        self.db.movies.get(id)
    }

    /// Get list of series.
    pub(crate) fn series_by_name(&self) -> impl DoubleEndedIterator<Item = &Series> {
        self.db.series.iter_by_name()
    }

    /// Get list of series.
    pub(crate) fn movies_by_name(&self) -> impl DoubleEndedIterator<Item = &Movie> {
        self.db.movies.iter_by_name()
    }

    /// Iterator over available episodes.
    #[inline]
    pub(crate) fn episodes(
        &self,
        id: &SeriesId,
    ) -> impl DoubleEndedIterator<Item = EpisodeRef<'_>> + ExactSizeIterator {
        self.db.episodes.by_series(id)
    }

    /// Iterator over all episodes in a given season.
    #[inline]
    pub(crate) fn episodes_by_season(
        &self,
        id: &SeriesId,
        season: &SeasonNumber,
    ) -> impl DoubleEndedIterator<Item = EpisodeRef<'_>> + ExactSizeIterator {
        self.db.episodes.by_season(id, season)
    }

    /// Get reference to an episode.
    #[inline]
    pub(crate) fn episode(&self, id: &EpisodeId) -> Option<EpisodeRef<'_>> {
        self.db.episodes.get(id)
    }

    /// Get a single season.
    #[inline]
    pub(crate) fn season(
        &self,
        series_id: &SeriesId,
        season: &SeasonNumber,
    ) -> Option<SeasonRef<'_>> {
        self.db.seasons.get(series_id, season)
    }

    /// Iterator over available seasons.
    #[inline]
    pub(crate) fn seasons(
        &self,
        series_id: &SeriesId,
    ) -> impl DoubleEndedIterator<Item = SeasonRef<'_>> + ExactSizeIterator + Clone {
        self.db.seasons.by_series(series_id)
    }

    /// Get all the watches for the given episode.
    #[inline]
    pub(crate) fn watched_by_episode(
        &self,
        episode_id: &EpisodeId,
    ) -> impl ExactSizeIterator<Item = &Watched> + DoubleEndedIterator + Clone {
        self.db.watched.by_episode(episode_id)
    }

    /// Get all the watches for the given movie.
    #[inline]
    pub(crate) fn watched_by_movie(
        &self,
        movie_id: &MovieId,
    ) -> impl ExactSizeIterator<Item = &Watched> + DoubleEndedIterator + Clone {
        self.db.watched.by_movie(movie_id)
    }

    /// Get task queue.
    pub(crate) fn pending_tasks(&self) -> impl ExactSizeIterator<Item = &Task> {
        self.db.tasks.pending()
    }

    /// Get task queue.
    pub(crate) fn running_tasks(&self) -> impl ExactSizeIterator<Item = &Task> {
        self.db.tasks.running()
    }

    /// Get completed tasks.
    pub(crate) fn completed_tasks(&self) -> impl ExactSizeIterator<Item = &CompletedTask> {
        self.db.tasks.completed()
    }

    /// Get season summary statistics.
    pub(crate) fn season_watched(
        &self,
        series_id: &SeriesId,
        season: &SeasonNumber,
    ) -> (usize, usize) {
        let mut total = 0;
        let mut watched = 0;

        for episode in self.episodes(series_id).filter(|e| e.season == *season) {
            total += 1;
            watched += usize::from(self.watched_by_episode(&episode.id).len() != 0);
        }

        (watched, total)
    }

    /// Get the pending episode for the given movie.
    pub(crate) fn pending_by_movie(&self, movie_id: &MovieId) -> Option<&Pending> {
        self.db.pending.by_movie(movie_id)
    }

    /// Return list of pending episodes.
    pub(crate) fn pending(&self) -> impl DoubleEndedIterator<Item = PendingRef<'_>> + Clone {
        self.db
            .pending
            .iter()
            .flat_map(move |p| self.pending_ref(p))
    }

    /// Get pending by series.
    pub(crate) fn pending_ref_by_series(&self, series_id: &SeriesId) -> Option<PendingRef<'_>> {
        let p = self.db.pending.get(series_id)?;
        self.pending_ref(p)
    }

    fn pending_ref(&self, p: &Pending) -> Option<PendingRef<'_>> {
        match &p.kind {
            PendingKind::Episode { series, episode } => {
                let series = self.db.series.get(series)?;

                if !series.tracked {
                    return None;
                }

                let episode = self.db.episodes.get(episode)?;
                let season = self.season(&series.id, &episode.season);

                Some(PendingRef::Episode {
                    series,
                    season,
                    episode,
                })
            }
            PendingKind::Movie { movie } => {
                let movie = self.db.movies.get(movie)?;

                Some(PendingRef::Movie { movie })
            }
        }
    }

    /// Test if we have changes.
    pub(crate) fn has_changes(&self) -> bool {
        self.db.changes.has_changes()
    }

    /// Find updates that need to be performed.
    pub(crate) fn find_updates(&mut self, now: &Timestamp) {
        for s in self.db.series.iter() {
            // Ignore series which are no longer tracked.
            if !s.tracked {
                continue;
            }

            let Some(remote_id) = s.remote_id else {
                continue;
            };

            // Reduce the number of API requests by ensuring we don't check for
            // updates more than each CACHE_TIME interval.
            if let Some(last_sync) = self.db.sync.last_sync(&remote_id) {
                if now.duration_since(*last_sync).as_secs() < CACHE_TIME {
                    continue;
                }
            }

            let last_modified = self.db.sync.last_modified(&remote_id).copied();

            if matches!(last_modified, Some(last_modified) if now.duration_since(last_modified).as_secs() < CACHE_TIME)
            {
                continue;
            }

            let kind = match remote_id {
                RemoteId::Tvdb { .. } => TaskKind::CheckForUpdates {
                    series_id: s.id,
                    remote_id,
                    last_modified,
                },
                RemoteId::Tmdb { .. } => TaskKind::DownloadSeries {
                    series_id: s.id,
                    remote_id,
                    last_modified,
                    force: false,
                },
                RemoteId::Imdb { .. } => continue,
            };

            self.db.tasks.push(now, kind);
        }

        for m in self.db.movies.iter() {
            let Some(remote_id) = m.remote_id else {
                continue;
            };

            // Reduce the number of API requests by ensuring we don't check for
            // updates more than each CACHE_TIME interval.
            if let Some(last_sync) = self.db.sync.last_sync(&remote_id) {
                if now.duration_since(*last_sync).as_secs() < CACHE_TIME {
                    continue;
                }
            }

            let last_modified = self.db.sync.last_modified(&remote_id).copied();

            if matches!(last_modified, Some(last_modified) if now.duration_since(last_modified).as_secs() < CACHE_TIME)
            {
                continue;
            }

            let kind = match remote_id {
                RemoteId::Tmdb { .. } => TaskKind::DownloadMovie {
                    movie_id: m.id,
                    remote_id,
                    last_modified,
                    force: false,
                },
                _ => continue,
            };

            self.db.tasks.push(now, kind);
        }
    }

    /// Check for update for the given series.
    pub(crate) fn check_for_updates(
        &mut self,
        series_id: SeriesId,
        remote_id: RemoteId,
        last_modified: Option<Timestamp>,
    ) -> impl Future<Output = Result<Option<TaskKind>>> {
        let tvdb = self.tvdb.clone();

        let future = async move {
            match remote_id {
                RemoteId::Tvdb { id } => {
                    let Some(update) = tvdb.series_last_modified(id).await? else {
                        bail!("{series_id}/{remote_id}: missing last-modified in api");
                    };

                    tracing::trace!(?update, ?last_modified, ?series_id, ?remote_id,);

                    if matches!(last_modified, Some(last_modified) if last_modified >= update) {
                        return Ok(None);
                    }

                    let kind = TaskKind::DownloadSeries {
                        series_id,
                        remote_id,
                        last_modified: Some(update),
                        force: false,
                    };

                    Ok(Some(kind))
                }
                // Nothing to do with the IMDB remote.
                remote_id => Ok(Some(TaskKind::DownloadSeries {
                    series_id,
                    remote_id,
                    last_modified,
                    force: false,
                })),
            }
        };

        future.in_current_span()
    }

    /// Push a single task to the queue.
    pub(crate) fn push_task_without_delay(&mut self, kind: TaskKind) -> bool {
        self.db.tasks.push_without_delay(kind)
    }

    /// Add updates to download to the queue.
    pub(crate) fn push_task(&mut self, now: &Timestamp, task: TaskKind) {
        self.db.tasks.push(now, task);
    }

    /// Mark an episode as watched at the given timestamp.
    pub(crate) fn watch_remaining_season(
        &mut self,
        now: &Zoned,
        series_id: &SeriesId,
        season: &SeasonNumber,
        remaining_season: RemainingSeason,
    ) {
        let today = now.date();
        let mut last = None;

        for episode in self
            .db
            .episodes
            .by_series(series_id)
            .filter(|e| e.season == *season)
        {
            if self.watched_by_episode(&episode.id).len() > 0 {
                continue;
            }

            // NB: only mark episodes which have actually aired.
            if !episode.has_aired(&today) {
                continue;
            }

            let timestamp = match remaining_season {
                RemainingSeason::Aired => now.timestamp(),
                RemainingSeason::AirDate => {
                    let Some(air_date) = episode.aired_timestamp() else {
                        continue;
                    };

                    air_date
                }
            };

            self.db.watched.insert(Watched {
                id: WatchedId::random(),
                timestamp,
                kind: WatchedKind::Series {
                    series: *series_id,
                    episode: episode.id,
                },
            });

            self.db.changes.change(Change::Watched);
            last = Some(episode.id);
        }

        if let Some(last) = last {
            self.populate_pending_from(now, series_id, &last);
        } else if self.db.pending.remove_series(series_id).is_some() {
            self.db.changes.change(Change::Pending);
        }
    }

    /// Mark an episode as watched.
    #[tracing::instrument(skip(self))]
    pub(crate) fn watch(
        &mut self,
        now: &Zoned,
        episode_id: &EpisodeId,
        remaining_season: RemainingSeason,
    ) {
        tracing::trace!("Marking as watched");

        let Some(episode) = self.db.episodes.get(episode_id) else {
            tracing::warn!(?episode_id, "Episode missing");
            return;
        };

        let timestamp = match remaining_season {
            RemainingSeason::Aired => now.timestamp(),
            RemainingSeason::AirDate => {
                let Some(air_date) = episode.aired_timestamp() else {
                    return;
                };

                air_date
            }
        };

        let series = *episode.series();
        let episode = episode.id;

        self.db.watched.insert(Watched {
            id: WatchedId::random(),
            timestamp,
            kind: WatchedKind::Series { series, episode },
        });

        self.db.changes.change(Change::Watched);
        self.populate_pending_from(now, &series, &episode);
    }

    /// Mark an episode as watched.
    #[tracing::instrument(skip(self))]
    pub(crate) fn watch_movie(
        &mut self,
        now: &Timestamp,
        movie: &MovieId,
        remaining_season: RemainingSeason,
    ) {
        tracing::trace!("Marking as watched");

        let Some(m) = self.db.movies.get(movie) else {
            tracing::warn!(?movie, "Movie missing");
            return;
        };

        let timestamp = match remaining_season {
            RemainingSeason::Aired => *now,
            RemainingSeason::AirDate => {
                let Some(release_date) = m.release() else {
                    return;
                };

                release_date
            }
        };

        self.db.watched.insert(Watched {
            id: WatchedId::random(),
            timestamp,
            kind: WatchedKind::Movie { movie: m.id },
        });

        self.db.changes.change(Change::Watched);

        if self.db.pending.remove_movie(movie).is_some() {
            self.db.changes.change(Change::Pending);
        }
    }

    /// Skip an episode.
    #[tracing::instrument(skip(self))]
    pub(crate) fn skip(&mut self, now: &Zoned, series_id: &SeriesId, id: &EpisodeId) {
        tracing::trace!("Skipping episode");
        self.populate_pending_from(now, series_id, id);
    }

    /// Skip an episode.
    #[tracing::instrument(skip(self))]
    pub(crate) fn skip_movie(&mut self, now: &Timestamp, id: &MovieId) {
        tracing::trace!("Skipping movie");
        self.db.pending.remove_movie(id);
    }

    /// Select the next pending episode to use for a show.
    #[tracing::instrument(skip(self))]
    pub(crate) fn select_pending(&mut self, now: &Timestamp, episode_id: &EpisodeId) {
        tracing::trace!("Selecting pending series");

        let Some(episode) = self.db.episodes.get(episode_id) else {
            tracing::warn!("Episode missing");
            return;
        };

        let aired = self
            .db
            .episodes
            .get(episode_id)
            .and_then(|e| e.aired_timestamp());

        let timestamp = self
            .db
            .watched
            .by_series(episode.series())
            .next_back()
            .map(|w| w.timestamp);

        self.db.pending.extend([Pending {
            timestamp: pending_timestamp(now, &[timestamp, aired]),
            kind: PendingKind::Episode {
                series: *episode.series(),
                episode: episode.id,
            },
        }]);

        self.db.changes.change(Change::Pending);
    }

    /// Select the next pending movie.
    #[tracing::instrument(skip(self))]
    pub(crate) fn select_pending_movie(&mut self, now: &Timestamp, movie_id: &MovieId) {
        tracing::trace!("Selecting pending movie");

        let m = self.db.movies.get(movie_id);
        let earliest = m.and_then(|m| m.earliest());

        if self.db.watched.by_movie(movie_id).len() == 0 {
            let timestamp = pending_timestamp(now, &[earliest]);

            self.db.pending.extend([Pending {
                timestamp,
                kind: PendingKind::Movie { movie: *movie_id },
            }]);

            self.db.changes.change(Change::Pending);
        }
    }

    /// Clear next episode as pending.
    #[tracing::instrument(skip(self))]
    pub(crate) fn clear_pending(&mut self, episode_id: &EpisodeId) {
        tracing::trace!("Clearing pending");

        self.db.changes.change(Change::Pending);

        if let Some(e) = self.db.episodes.get(episode_id) {
            self.db.pending.remove_series(e.series());
        }
    }

    /// Clear next episode as pending.
    #[tracing::instrument(skip(self))]
    pub(crate) fn clear_pending_movie(&mut self, movie_id: &MovieId) {
        tracing::trace!("Clearing pending movie");

        self.db.changes.change(Change::Pending);
        self.db.pending.remove_movie(movie_id);
    }

    /// Remove a watch of the given episode.
    #[tracing::instrument(skip(self))]
    pub(crate) fn remove_episode_watch(&mut self, episode_id: &EpisodeId, watch_id: &WatchedId) {
        tracing::trace!("Removing episode watch");

        let Some(w) = self.db.watched.remove_watch(watch_id) else {
            tracing::warn!("Watch missing");
            return;
        };

        self.db.changes.change(Change::Watched);

        if let Some(e) = self.db.episodes.get(episode_id) {
            if self.db.watched.by_episode(&e.id).len() == 0 {
                self.db.pending.extend([Pending {
                    timestamp: w.timestamp,
                    kind: PendingKind::Episode {
                        series: *e.series(),
                        episode: e.id,
                    },
                }]);

                self.db.changes.change(Change::Pending);
            }
        }
    }

    /// Remove a single watch for the given movie.
    #[tracing::instrument(skip(self))]
    pub(crate) fn remove_movie_watch(&mut self, movie_id: &MovieId, watch_id: &WatchedId) {
        tracing::trace!("Removing episode watch");

        let Some(..) = self.db.watched.remove_watch(watch_id) else {
            tracing::warn!("Watch missing");
            return;
        };

        self.db.changes.change(Change::Watched);
        // if let Some(m) = self.db.movies.get(movie_id) {
        //     if self.db.watched.by_movie(&m.id).len() == 0 {
        //         self.db.pending.extend([Pending {
        //             series: *m.series(),
        //             episode: m.id,
        //             timestamp: w.timestamp,
        //         }]);

        //         self.db.changes.change(Change::Pending);
        //     }
        // }
    }

    /// Remove all watches of the given episode.
    #[tracing::instrument(skip(self))]
    pub(crate) fn remove_season_watches(
        &mut self,
        now: &Timestamp,
        series_id: &SeriesId,
        season: &SeasonNumber,
    ) {
        tracing::trace!("Removing season watches");

        let mut removed = 0;

        for e in self.db.episodes.by_series(series_id) {
            if e.season == *season {
                removed += self.db.watched.remove_by_episode(&e.id);
            }
        }

        if removed > 0 {
            self.db.changes.change(Change::Watched);
        }

        if self.db.pending.remove_series(series_id).is_some() {
            self.db.changes.change(Change::Pending);
        }

        // Find the first episode matching the cleared season.
        if let Some(e) = self
            .db
            .episodes
            .by_series(series_id)
            .find(|e| e.season == *season)
        {
            let timestamp = self
                .db
                .watched
                .by_series(series_id)
                .next_back()
                .map(|w| w.timestamp);

            self.db.pending.extend([Pending {
                timestamp: pending_timestamp(now, &[timestamp, e.aired_timestamp()]),
                kind: PendingKind::Episode {
                    series: *series_id,
                    episode: e.id,
                },
            }]);

            self.db.changes.change(Change::Pending);
        }
    }

    /// Save changes made.
    #[tracing::instrument(skip(self))]
    pub(crate) fn save_changes(&mut self) -> impl Future<Output = Result<()>> {
        if self.db.changes.contains(Change::Series) || self.db.changes.contains(Change::Schedule) {
            self.rebuild_schedule();
        }

        self.db
            .save_changes(&self.paths, self.do_not_save)
            .in_current_span()
    }

    /// Populate pending from a series where we don't know which episode to
    /// populate from.
    #[tracing::instrument(skip(self))]
    pub(crate) fn populate_pending(&mut self, now: &Timestamp, id: &SeriesId) {
        tracing::trace!("Populate pending");

        if let Some(pending) = self.db.pending.get(id) {
            // Do nothing since we already have a pending episode.
            tracing::trace!(?pending, "pending exists");
            return;
        }

        let last = self.db.watched.by_series(id).next_back();

        let mut cur = if let Some(WatchedKind::Series { episode, .. }) = last.map(|w| &w.kind) {
            tracing::trace!(?episode, "Episode after watched");
            self.db.episodes.get(episode).and_then(EpisodeRef::next)
        } else {
            tracing::trace!("Finding next unwatched episode");
            self.db.episodes.by_series(id).next()
        };

        while let Some(e) = cur {
            if !e.season.is_special() && self.db.watched.by_episode(&e.id).len() == 0 {
                break;
            }

            cur = e.next();
        }

        let Some(e) = cur else {
            return;
        };

        tracing::trace!(episode = ?e.id, "Set pending");

        self.db.changes.change(Change::Pending);
        // Mark the next episode in the show as pending.
        self.db.pending.extend([Pending {
            timestamp: pending_timestamp(now, &[last.map(|w| w.timestamp), e.aired_timestamp()]),
            kind: PendingKind::Episode {
                series: *id,
                episode: e.id,
            },
        }]);
    }

    /// Populate pending from a known episode ID.
    fn populate_pending_from(&mut self, now: &Zoned, series_id: &SeriesId, id: &EpisodeId) {
        let Some(e) = self.db.episodes.get(id).and_then(|e| e.next()) else {
            if self.db.pending.remove_series(series_id).is_some() {
                self.db.changes.change(Change::Pending);
            }

            return;
        };

        self.db.changes.change(Change::Pending);
        let timestamp = e
            .aired_timestamp()
            .map(|t| t.max(now.timestamp()))
            .unwrap_or(now.timestamp());

        self.db.pending.extend([Pending {
            timestamp,
            kind: PendingKind::Episode {
                series: *series_id,
                episode: e.id,
            },
        }]);
    }

    /// Get current configuration.
    pub(crate) fn config(&self) -> &Config {
        &self.db.config
    }

    /// Get configuration mutably indicating that it has been changed.
    pub(crate) fn config_mut(&mut self) -> &mut Config {
        self.db.changes.change(Change::Config);
        &mut self.db.config
    }

    /// Get the current theme.
    pub(crate) fn theme(&self) -> &ThemeType {
        &self.db.config.theme
    }

    /// Set the theme configuration option.
    pub(crate) fn set_theme(&mut self, theme: ThemeType) {
        self.db.config.theme = theme;
        self.db.changes.change(Change::Config);
    }

    /// Set the theme configuration option.
    pub(crate) fn set_tvdb_legacy_api_key(&mut self, api_key: String) {
        self.tvdb.set_api_key(&api_key);
        self.db.config.tvdb_legacy_apikey.set(api_key);
        self.db.changes.change(Change::Config);
    }

    /// Set the theme configuration option.
    pub(crate) fn set_tmdb_api_key(&mut self, api_key: String) {
        self.tmdb.set_api_key(&api_key);
        self.db.config.tmdb_api_key.set(api_key);
        self.db.changes.change(Change::Config);
    }

    /// Check if series is tracked.
    pub(crate) fn get_series_by_remote(&self, id: &RemoteId) -> Option<&Series> {
        let id = self.db.remotes.get_series(id)?;
        self.db.series.get(&id)
    }

    /// Check if movie is tracked.
    pub(crate) fn get_movie_by_remote(&self, id: &RemoteId) -> Option<&Movie> {
        let id = self.db.remotes.get_movie(id)?;
        self.db.movies.get(&id)
    }

    /// Remove the given series.
    #[tracing::instrument(skip(self))]
    pub(crate) fn remove_series(&mut self, id: &SeriesId) {
        tracing::info!("Remove series");

        let _ = self.db.series.remove(id);
        self.db.episodes.remove(id);
        self.db.seasons.remove(id);
        self.db.changes.remove_series(id);
        self.db.tasks.remove_tasks_by(|t| t.is_series(id));
    }

    /// Remove the given movie.
    #[tracing::instrument(skip(self))]
    pub(crate) fn remove_movie(&mut self, id: &MovieId) {
        tracing::info!("Remove movie");

        let _ = self.db.movies.remove(id);
        self.db.changes.remove_movie(id);
        self.db.tasks.remove_tasks_by(|t| t.is_movie(id));
    }

    /// Download series using a remote identifier.
    #[tracing::instrument(skip(self))]
    pub(crate) fn download_series(
        &self,
        remote_id: &RemoteId,
        if_none_match: Option<&Etag>,
        series_id: Option<&SeriesId>,
    ) -> impl Future<Output = Result<Option<NewSeries>>> {
        let tvdb = self.tvdb.clone();
        let tmdb = self.tmdb.clone();
        let proxy = self.db.remotes.proxy();
        let remote_id = *remote_id;
        let if_none_match = if_none_match.cloned();
        let series_id = series_id.copied();

        let future = async move {
            tracing::info!("Downloading series");

            let lookup_series = |q| {
                if let Some(series_id) = series_id {
                    return Some(series_id);
                }

                proxy.find_series_by_remote(q)
            };

            let lookup_episode = |q| proxy.find_episode_by_remote(q);

            let data = match remote_id {
                RemoteId::Tvdb { id } => {
                    let series = tvdb.series(id, lookup_series);
                    let episodes = tvdb.series_episodes(id, lookup_episode);
                    let ((series, remote_ids, last_etag, last_modified), episodes) =
                        tokio::try_join!(series, episodes)?;
                    let seasons = episodes_into_seasons(&episodes);

                    NewSeries {
                        series,
                        remote_ids,
                        last_etag,
                        last_modified,
                        episodes,
                        seasons,
                    }
                }
                RemoteId::Tmdb { id } => {
                    let Some((series, remote_ids, last_etag, last_modified, seasons)) = tmdb
                        .series(id, lookup_series, if_none_match.as_ref())
                        .await?
                    else {
                        tracing::trace!("{remote_id}: not changed");
                        return Ok(None);
                    };

                    let mut episodes = Vec::new();

                    for season in &seasons {
                        let new_episodes = tmdb
                            .download_episodes(
                                id,
                                season.number,
                                series.language.as_deref(),
                                &lookup_episode,
                            )
                            .await?;

                        episodes.extend(new_episodes);
                    }

                    NewSeries {
                        series,
                        remote_ids,
                        last_etag,
                        last_modified,
                        episodes,
                        seasons,
                    }
                }
                RemoteId::Imdb { .. } => {
                    bail!("Cannot download series data from IMDB")
                }
            };

            Ok::<_, Error>(Some(data))
        };

        future.in_current_span()
    }

    /// Download series using a remote identifier.
    #[tracing::instrument(skip(self))]
    pub(crate) fn download_movie(
        &self,
        remote_id: &RemoteId,
        if_none_match: Option<&Etag>,
        movie_id: Option<&MovieId>,
    ) -> impl Future<Output = Result<Option<NewMovie>>> {
        let tmdb = self.tmdb.clone();
        let proxy = self.db.remotes.proxy();
        let remote_id = *remote_id;
        let if_none_match = if_none_match.cloned();
        let movie_id = movie_id.copied();

        let future = async move {
            tracing::info!("Downloading movies");

            let lookup_movie = |q| {
                if let Some(movie_id) = movie_id {
                    return Some(movie_id);
                }

                proxy.find_movie_by_remote(q)
            };

            let data = match remote_id {
                RemoteId::Tmdb { id } => {
                    let Some((movie, remote_ids, last_etag, last_modified)) =
                        tmdb.movie(id, lookup_movie, if_none_match.as_ref()).await?
                    else {
                        tracing::trace!("{remote_id}: not changed");
                        return Ok(None);
                    };

                    NewMovie {
                        movie,
                        remote_ids,
                        last_etag,
                        last_modified,
                    }
                }
                RemoteId::Tvdb { .. } => {
                    bail!("Cannot download movie data from tvdb")
                }
                RemoteId::Imdb { .. } => {
                    bail!("Cannot download movie data from imdb")
                }
            };

            Ok::<_, Error>(Some(data))
        };

        future.in_current_span()
    }

    /// If the series is already loaded in the local database, simply mark it as tracked.
    pub(crate) fn is_series_by_remote(&mut self, id: &RemoteId) -> bool {
        let Some(id) = self.db.remotes.get_series(id) else {
            return false;
        };

        self.track(&id)
    }

    /// Test if a movie with the given remote exists.
    pub(crate) fn is_movie_by_remote(&mut self, id: &RemoteId) -> bool {
        let Some(id) = self.db.remotes.get_movie(id) else {
            return false;
        };

        self.db.movies.get_mut(&id).is_some()
    }

    /// Set the given show as tracked.
    pub(crate) fn track(&mut self, series_id: &SeriesId) -> bool {
        let Some(series) = self.db.series.get_mut(series_id) else {
            return false;
        };

        series.tracked = true;
        self.db.changes.change(Change::Series);
        true
    }

    /// Disable tracking of the series with the given id.
    pub(crate) fn untrack(&mut self, series_id: &SeriesId) {
        if let Some(s) = self.db.series.get_mut(series_id) {
            s.tracked = false;
            self.db.changes.change(Change::Series);
        }
    }

    /// Insert a new tracked series
    #[tracing::instrument(skip(self))]
    pub(crate) fn insert_series(&mut self, now: &Timestamp, data: NewSeries) {
        tracing::info!("Inserting new series");

        let series_id = data.series.id;

        for &remote_id in &data.remote_ids {
            if self.db.remotes.insert_series(remote_id, series_id) {
                self.db.changes.change(Change::Remotes);
            }
        }

        if self
            .db
            .sync
            .update_last_etag(data.series.remote_id, data.last_etag)
        {
            self.db.changes.change(Change::Sync);
        }

        if self
            .db
            .sync
            .update_last_modified(data.series.remote_id, data.last_modified)
        {
            self.db.changes.change(Change::Sync);
        }

        let mut episodes = Vec::with_capacity(data.episodes.len());

        for episode in data.episodes {
            for &remote_id in &episode.remote_ids {
                if self
                    .db
                    .remotes
                    .insert_episode(remote_id, episode.episode.id)
                {
                    self.db.changes.change(Change::Remotes);
                }
            }

            episodes.push(episode.episode);
        }

        self.db.episodes.insert(series_id, episodes);
        self.db.seasons.insert(series_id, data.seasons.clone());

        if let Some(current) = self.db.series.get_mut(&series_id) {
            current.merge_from(data.series);
        } else {
            self.db.series.insert(Series::new_series(data.series));
        }

        // Remove any pending episodes for the given series.
        self.populate_pending(now, &series_id);
        self.db.changes.add_series(&series_id);
    }

    /// Insert a new tracked movie.
    #[tracing::instrument(skip(self))]
    pub(crate) fn insert_movie(&mut self, now: &Timestamp, data: NewMovie) {
        tracing::info!("Inserting new movie");

        let movie_id = data.movie.id;

        for &remote_id in &data.remote_ids {
            if self.db.remotes.insert_movie(remote_id, movie_id) {
                self.db.changes.change(Change::Remotes);
            }
        }

        if self
            .db
            .sync
            .update_last_etag(data.movie.remote_id, data.last_etag)
        {
            self.db.changes.change(Change::Sync);
        }

        if self
            .db
            .sync
            .update_last_modified(data.movie.remote_id, data.last_modified)
        {
            self.db.changes.change(Change::Sync);
        }

        if let Some(current) = self.db.movies.get_mut(&movie_id) {
            current.merge_from(data.movie);
        } else {
            self.db.movies.insert(Movie::new_movie(data.movie));
        }

        self.db.changes.add_movie(&movie_id);
        self.select_pending_movie(now, &movie_id);
    }

    /// Ensure that a collection of the given image ids are loaded.
    pub(crate) fn load_image(
        &self,
        image: ImageV2,
        hint: ImageHint,
    ) -> impl Future<Output = Result<PathBuf>> {
        tracing::info!(?image, ?hint, "loading image");

        let paths = self.paths.clone();
        let tvdb = self.tvdb.clone();
        let tmdb = self.tmdb.clone();

        let paths = paths.clone();
        let tvdb = tvdb.clone();
        let tmdb = tmdb.clone();

        let future = async move {
            let hash = image.hash();

            let handle = match &image {
                ImageV2::Tvdb { uri } => {
                    cache::image(&paths.images, &tvdb, uri.as_ref(), hash, hint).await
                }
                ImageV2::Tmdb { uri } => {
                    cache::image(&paths.images, &tmdb, uri.as_ref(), hash, hint).await
                }
                image => Err(anyhow!("Unsupported image type: {image}")),
            };

            let handle = handle.with_context(|| anyhow!("Downloading: {image:?}"))?;
            Ok::<_, Error>(handle)
        };

        future.in_current_span()
    }

    /// Prevents the service from saving anything to the filesystem.
    pub fn do_not_save(&mut self) {
        self.do_not_save = true;
    }

    /// Get existing id by remote if it exists.
    pub(crate) fn existing_by_remote_ids<I>(&self, ids: I) -> Option<SeriesId>
    where
        I: IntoIterator<Item = RemoteId>,
    {
        for remote_id in ids {
            if let Some(id) = self.db.remotes.get_series(&remote_id) {
                return Some(id);
            }
        }

        None
    }

    /// Insert a new watch.
    pub(crate) fn insert_new_watch(
        &mut self,
        series_id: SeriesId,
        episode_id: EpisodeId,
        timestamp: Timestamp,
    ) {
        self.db.watched.insert(Watched {
            id: WatchedId::random(),
            timestamp,
            kind: WatchedKind::Series {
                series: series_id,
                episode: episode_id,
            },
        });

        self.db.changes.change(Change::Watched);
    }

    /// Remove watch history matching the given series.
    pub(crate) fn clear_watches(&mut self, series_id: &SeriesId) {
        self.db.watched.remove_by_series(series_id);
        self.db.changes.change(Change::Watched);
    }

    /// Find an episode using the given predicate.
    pub(crate) fn find_episode_by<P>(
        &self,
        series_id: &SeriesId,
        mut predicate: P,
    ) -> Option<EpisodeRef<'_>>
    where
        P: FnMut(&Episode) -> bool,
    {
        self.episodes(series_id).find(move |e| predicate(e))
    }

    /// Search tvdb.
    pub(crate) fn search_tvdb(
        &self,
        query: &str,
    ) -> impl Future<Output = Result<Vec<SearchSeries>>> {
        let tvdb = self.tvdb.clone();
        let query = query.to_owned();
        async move { tvdb.search_by_name(&query).await }.in_current_span()
    }

    /// Search series from tmdb.
    pub(crate) fn search_series_tmdb(
        &self,
        query: &str,
    ) -> impl Future<Output = Result<Vec<SearchSeries>>> {
        let tmdb = self.tmdb.clone();
        let query = query.to_owned();
        async move { tmdb.search_series(&query).await }.in_current_span()
    }

    /// Search movies from tmdb.
    pub(crate) fn search_movies_tmdb(
        &self,
        query: &str,
    ) -> impl Future<Output = Result<Vec<SearchMovie>>> {
        let tmdb = self.tmdb.clone();
        let query = query.to_owned();
        async move { tmdb.search_movies(&query).await }.in_current_span()
    }

    /// Build schedule information.
    #[tracing::instrument(skip(self))]
    pub(crate) fn rebuild_schedule(&mut self) {
        tracing::trace!("Rebuilding schedule");

        let mut current = self.now;

        let mut days = Vec::new();

        while (current.duration_since(self.now).as_hours() / 24).unsigned_abs()
            <= self.config().schedule_duration_days
        {
            let zoned = current.to_zoned(TimeZone::UTC);

            let mut schedule = Vec::new();

            for series in self.db.series.iter() {
                if !series.tracked {
                    continue;
                }

                let mut scheduled_episodes = Vec::new();

                for e in self.episodes(&series.id) {
                    let Some(air_date) = e.aired else {
                        continue;
                    };

                    if air_date != zoned.date() {
                        continue;
                    }

                    scheduled_episodes.push(e.id);
                }

                if !scheduled_episodes.is_empty() {
                    schedule.push(ScheduledSeries {
                        series_id: series.id,
                        episodes: scheduled_episodes,
                    });
                }
            }

            if !schedule.is_empty() {
                days.push(ScheduledDay {
                    date: zoned.date(),
                    schedule,
                });
            }

            let Ok(next) = current.checked_add(1.days()) else {
                break;
            };

            current = next;
        }

        self.schedule = days;
    }

    /// Take if a queue has been modified.
    #[inline]
    pub(crate) fn take_tasks_modified(&mut self) -> bool {
        self.db.tasks.take_modified()
    }

    /// Get the next task in the queue.
    #[inline]
    pub(crate) fn next_task(&mut self, now: &Timestamp, timed_out: Option<TaskId>) -> Option<Task> {
        self.db.tasks.next_task(now, timed_out)
    }

    /// Next duration to sleep.
    #[inline]
    pub(crate) fn next_task_sleep(&self, now: &Timestamp) -> Option<(u64, TaskId)> {
        self.db.tasks.next_sleep(now)
    }

    /// Check if the given task is pending.
    #[inline]
    pub(crate) fn task_status(&self, id: TaskRef) -> Option<TaskStatus> {
        self.db.tasks.status(id)
    }

    /// Check if the given task is pending.
    #[inline]
    pub(crate) fn task_status_any(
        &self,
        ids: impl IntoIterator<Item = TaskRef>,
    ) -> Option<TaskStatus> {
        ids.into_iter()
            .flat_map(|id| self.db.tasks.status(id))
            .next()
    }

    /// Mark task as completed.
    #[inline]
    pub(crate) fn complete_task(&mut self, now: &Timestamp, task: Task) -> Option<TaskStatus> {
        if let TaskKind::CheckForUpdates {
            remote_id,
            last_modified,
            ..
        }
        | TaskKind::DownloadSeries {
            remote_id,
            last_modified,
            ..
        }
        | TaskKind::DownloadMovie {
            remote_id,
            last_modified,
            ..
        } = &task.kind
        {
            if self.db.sync.update_sync(*remote_id, *now, *last_modified) {
                self.db.changes.change(Change::Sync);
            }
        }

        self.db.tasks.complete(now, task)
    }

    /// Get remotes by series.
    pub(crate) fn remotes_by_series(&self, id: &SeriesId) -> impl Iterator<Item = RemoteId> + '_ {
        self.db.remotes.get_by_series(id)
    }

    /// Get remotes by movie.
    pub(crate) fn remotes_by_movie(&self, id: &MovieId) -> impl Iterator<Item = RemoteId> + '_ {
        self.db.remotes.get_by_movie(id)
    }

    /// Clear last sync.
    pub(crate) fn clear_sync(&mut self) {
        self.db.sync.clear();
    }

    /// Get last etag for the given series id.
    pub(crate) fn last_etag(&self, remote_id: &RemoteId) -> Option<&Etag> {
        self.db.sync.last_etag(remote_id)
    }
}

/// Helper to build seasons out of known episodes.
fn episodes_into_seasons(episodes: &[NewEpisode]) -> Vec<Season> {
    let mut map = BTreeMap::new();

    for NewEpisode { episode, .. } in episodes {
        let season = map.entry(episode.season).or_insert_with(|| Season {
            number: episode.season,
            ..Season::default()
        });

        season.air_date = match (season.air_date, episode.aired) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (Some(t), _) | (_, Some(t)) => Some(t),
            _ => None,
        };
    }

    map.into_values().collect()
}

/// Calculate pending timestamp.
fn pending_timestamp(now: &Timestamp, candidates: &[Option<Timestamp>]) -> Timestamp {
    if let Some(timestamp) = candidates.iter().flatten().max() {
        *timestamp
    } else {
        *now
    }
}

/// Mode for marking remaining season.
#[derive(Debug)]
pub(crate) enum RemainingSeason {
    /// Timestamp should be right now, but only if an episode has aired.
    Aired,
    /// Timestamp should be the air date of the episode.
    AirDate,
}
