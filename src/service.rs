pub(crate) mod paths;

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fmt;
use std::future::Future;
use std::path::Path;
use std::sync::Arc;

use anyhow::{anyhow, bail, Context, Error, Result};
use chrono::{DateTime, Days, Local, NaiveDate, Utc};
use futures::stream::FuturesUnordered;
use iced::Theme;
use iced_native::image::Handle;
use uuid::Uuid;

use crate::api::themoviedb;
use crate::api::thetvdb;
use crate::assets::ImageKey;
use crate::cache::{self};
use crate::database::{Change, Database, EpisodeRef, SeasonRef};
use crate::model::{
    Config, Episode, EpisodeId, Etag, ImageV2, Movie, MovieId, Pending, RemoteEpisodeId,
    RemoteMovieId, RemoteSeriesId, ScheduledDay, ScheduledSeries, SearchMovie, SearchSeries,
    Season, SeasonNumber, Series, SeriesId, Task, TaskId, TaskKind, ThemeType, Watched,
    WatchedKind,
};
use crate::queue::TaskStatus;

/// Data encapsulating a newly added series.
#[derive(Clone)]
pub(crate) struct NewSeries {
    series: Series,
    remote_ids: BTreeSet<RemoteSeriesId>,
    last_etag: Option<Etag>,
    last_modified: Option<DateTime<Utc>>,
    episodes: Vec<NewEpisode>,
    seasons: Vec<Season>,
}

/// New episode.
#[derive(Debug, Clone)]
pub(crate) struct NewEpisode {
    pub(crate) episode: Episode,
    pub(crate) remote_ids: BTreeSet<RemoteEpisodeId>,
}

impl NewSeries {
    /// Return the identifier of the newly downloaded series.
    pub(crate) fn series_id(&self) -> &SeriesId {
        &self.series.id
    }
}

impl fmt::Debug for NewSeries {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NewSeries").finish_non_exhaustive()
    }
}

/// A pending thing to watch.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PendingRef<'a> {
    pub(crate) series: &'a Series,
    pub(crate) season: Option<SeasonRef<'a>>,
    pub(crate) episode: EpisodeRef<'a>,
}
impl<'a> PendingRef<'a> {
    /// Get poster for the given pending reference.
    pub(crate) fn poster(&self) -> Option<&'a ImageV2> {
        if let Some(season) = self.season.map(|s| s.into_season()) {
            if let Some(image) = season.poster() {
                return Some(image);
            }
        }

        self.series.poster()
    }

    /// Test if episode will air in the future.
    pub(crate) fn will_air(&self, today: &NaiveDate) -> bool {
        self.episode.will_air(today)
    }

    /// Test if pending ref has aired.
    pub(crate) fn has_aired(&self, today: &NaiveDate) -> bool {
        self.episode.has_aired(today)
    }
}

/// Background service taking care of all state handling.
pub struct Service {
    paths: Arc<paths::Paths>,
    db: Database,
    tvdb: thetvdb::Client,
    tmdb: themoviedb::Client,
    do_not_save: bool,
    current_theme: Theme,
    schedule: Vec<ScheduledDay>,
    now: NaiveDate,
}

impl Service {
    /// Construct and setup in-memory state of
    pub fn new(config: &Path, cache: &Path) -> Result<Self> {
        let paths = paths::Paths::new(config, cache);

        if !paths.images.is_dir() {
            tracing::debug!("creating images directory: {}", paths.images.display());
            std::fs::create_dir_all(&paths.images)?;
        }

        let db = Database::load(&paths)?;
        let tvdb = thetvdb::Client::new(&db.config.tvdb_legacy_apikey)?;
        let tmdb = themoviedb::Client::new(&db.config.tmdb_api_key)?;

        let current_theme = db.config.theme();

        let now = Local::now();

        let mut this = Self {
            paths: Arc::new(paths),
            db,
            tvdb,
            tmdb,
            do_not_save: false,
            current_theme,
            schedule: Vec::new(),
            now: now.date_naive(),
        };

        this.build_schedule();
        Ok(this)
    }

    /// Naive date.
    pub(crate) fn now(&self) -> &NaiveDate {
        &self.now
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
    pub(crate) fn movie(&self, _: &MovieId) -> Option<&Movie> {
        // TODO: implement this
        None
    }

    /// Get list of series.
    pub(crate) fn series_by_name(&self) -> impl DoubleEndedIterator<Item = &Series> {
        self.db.series.iter_by_name()
    }

    /// Iterator over available episodes.
    #[inline]
    pub(crate) fn episodes(
        &self,
        id: &SeriesId,
    ) -> impl DoubleEndedIterator<Item = EpisodeRef<'_>> + ExactSizeIterator {
        self.db.episodes.by_series(id)
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
    pub(crate) fn watched(
        &self,
        episode_id: &EpisodeId,
    ) -> impl ExactSizeIterator<Item = &Watched> + DoubleEndedIterator + Clone {
        self.db.watched.by_episode(episode_id)
    }

    /// Get task queue.
    pub(crate) fn tasks(&self) -> impl ExactSizeIterator<Item = &Task> {
        self.db.tasks.pending()
    }

    /// Get task queue.
    pub(crate) fn running_tasks(&self) -> impl ExactSizeIterator<Item = &Task> {
        self.db.tasks.running()
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
            watched += usize::from(self.watched(&episode.id).len() != 0);
        }

        (watched, total)
    }

    /// Get the pending episode for the given series.
    pub(crate) fn get_pending(&self, series_id: &SeriesId) -> Option<&Pending> {
        self.db.pending.by_series(series_id)
    }

    /// Return list of pending episodes.
    pub(crate) fn pending(&self) -> impl DoubleEndedIterator<Item = PendingRef<'_>> + Clone {
        self.db
            .pending
            .iter()
            .flat_map(move |p| self.pending_ref(p))
    }

    /// Get pending by series.
    pub(crate) fn pending_by_series(&self, series_id: &SeriesId) -> Option<PendingRef<'_>> {
        let p = self.db.pending.get(series_id)?;
        self.pending_ref(p)
    }

    fn pending_ref(&self, p: &Pending) -> Option<PendingRef<'_>> {
        let series = self.db.series.get(&p.series)?;

        if !series.tracked {
            return None;
        }

        let episode = self.db.episodes.get(&p.episode)?;
        let season = self.season(&p.series, &episode.season);

        Some(PendingRef {
            series,
            season,
            episode,
        })
    }

    /// Test if we have changes.
    pub(crate) fn has_changes(&self) -> bool {
        self.db.changes.has_changes()
    }

    /// Find updates that need to be performed.
    pub(crate) fn find_updates(&mut self, now: &DateTime<Utc>) {
        // Cache series updates for 6 hours.
        const CACHE_TIME: i64 = 3600 * 6;

        for s in self.db.series.iter_mut() {
            if self.db.tasks.at_soft_capacity() {
                break;
            }

            // Ignore series which are no longer tracked.
            if !s.tracked {
                continue;
            }

            let Some(remote_id) = s.remote_id else {
                continue;
            };

            // Reduce the number of API requests by ensuring we don't check for
            // updates more than each CACHE_TIME interval.
            if let Some(last_sync) = self.db.sync.last_sync(&s.id, &remote_id) {
                if now.signed_duration_since(*last_sync).num_seconds() < CACHE_TIME {
                    continue;
                }
            }

            let kind = match remote_id {
                RemoteSeriesId::Tvdb { .. } => TaskKind::CheckForUpdates {
                    series_id: s.id,
                    remote_id,
                },
                RemoteSeriesId::Tmdb { .. } => TaskKind::DownloadSeries {
                    series_id: s.id,
                    remote_id,
                    last_modified: None,
                    force: false,
                },
                RemoteSeriesId::Imdb { .. } => continue,
            };

            if self.db.tasks.push(kind) {
                self.db.changes.change(Change::Series);
                self.db.changes.change(Change::Queue);
            }
        }
    }

    /// Check for update for the given series.
    pub(crate) fn check_for_updates(
        &mut self,
        series_id: &SeriesId,
        remote_id: &RemoteSeriesId,
    ) -> Option<impl Future<Output = Result<Option<TaskKind>>>> {
        let Some(s) = self.db.series.get(series_id) else {
            return None;
        };

        if let Some(RemoteSeriesId::Tmdb { .. }) = &s.remote_id {
            let kind = TaskKind::DownloadSeries {
                series_id: s.id,
                remote_id: *remote_id,
                last_modified: None,
                force: false,
            };

            if self.db.tasks.push(kind) {
                self.db.changes.change(Change::Queue);
            }

            return None;
        }

        let last_modified = self.db.sync.last_modified(series_id, remote_id).cloned();
        let tvdb = self.tvdb.clone();

        let series_id = s.id;
        let remote_id = *remote_id;

        Some(async move {
            let last_modified = match remote_id {
                RemoteSeriesId::Tvdb { id } => {
                    let Some(update) = tvdb.series_last_modified(id).await? else {
                        bail!("{series_id}/{remote_id}: missing last-modified in api");
                    };

                    tracing::trace!(
                        "{series_id}/{remote_id}: last modified {update:?} (existing {last_modified:?})"
                    );

                    if matches!(last_modified, Some(last_modified) if last_modified >= update) {
                        return Ok(None);
                    }

                    Some(update)
                }
                // Nothing to do with the IMDB remote.
                remote_id => bail!("{remote_id}: not supported for checking for updates"),
            };

            let kind = TaskKind::DownloadSeries {
                series_id,
                remote_id,
                last_modified,
                force: false,
            };

            Ok(Some(kind))
        })
    }

    /// Push a single task to the queue.
    pub(crate) fn push_task_without_delay(&mut self, kind: TaskKind) -> bool {
        if self.db.tasks.push_without_delay(kind) {
            self.db.changes.change(Change::Queue);
            true
        } else {
            false
        }
    }

    /// Add updates to download to the queue.
    pub(crate) fn push_tasks<I>(&mut self, it: I)
    where
        I: IntoIterator<Item = TaskKind>,
    {
        let mut any = false;

        for kind in it {
            any |= self.db.tasks.push(kind);
        }

        if any {
            self.db.changes.change(Change::Queue);
        }
    }

    /// Mark an episode as watched at the given timestamp.
    pub(crate) fn watch_remaining_season(
        &mut self,
        now: &DateTime<Utc>,
        series_id: &SeriesId,
        season: &SeasonNumber,
        remaining_season: RemainingSeason,
    ) {
        let today = now.date_naive();
        let mut last = None;

        for episode in self
            .db
            .episodes
            .by_series(series_id)
            .filter(|e| e.season == *season)
        {
            if self.watched(&episode.id).len() > 0 {
                continue;
            }

            // NB: only mark episodes which have actually aired.
            if !episode.has_aired(&today) {
                continue;
            }

            let timestamp = match remaining_season {
                RemainingSeason::Aired => *now,
                RemainingSeason::AirDate => {
                    let Some(air_date) = episode.aired_timestamp() else {
                        continue;
                    };

                    air_date
                }
            };

            self.db.watched.insert(Watched {
                id: Uuid::new_v4(),
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
        } else {
            self.remove_pending(series_id);
        }
    }

    /// Mark an episode as watched.
    pub(crate) fn watch(
        &mut self,
        now: &DateTime<Utc>,
        episode_id: &EpisodeId,
        remaining_season: RemainingSeason,
    ) {
        let Some(episode) = self.db.episodes.get(episode_id) else {
            return;
        };

        let timestamp = match remaining_season {
            RemainingSeason::Aired => *now,
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
            id: Uuid::new_v4(),
            timestamp,
            kind: WatchedKind::Series { series, episode },
        });

        self.db.changes.change(Change::Watched);
        self.populate_pending_from(now, &series, &episode);
    }

    /// Skip an episode.
    pub(crate) fn skip(
        &mut self,
        now: &DateTime<Utc>,
        series_id: &SeriesId,
        episode_id: &EpisodeId,
    ) {
        let Some(episode) = self.db.episodes.get(episode_id) else {
            if self.db.pending.remove(series_id).is_some() {
                self.db.changes.change(Change::Pending);
            }

            return;
        };

        self.db.changes.change(Change::Pending);
        self.db.pending.extend([Pending {
            series: *series_id,
            episode: episode.id,
            timestamp: *now,
        }]);
    }

    /// Select the next pending episode to use for a show.
    pub(crate) fn select_pending(&mut self, now: &DateTime<Utc>, episode_id: &EpisodeId) {
        let Some(episode) = self.db.episodes.get(episode_id) else {
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
            series: *episode.series(),
            episode: episode.id,
            timestamp: pending_timestamp(now, [timestamp, aired]),
        }]);

        self.db.changes.change(Change::Pending);
    }

    /// Clear next episode as pending.
    pub(crate) fn clear_pending(&mut self, episode_id: &EpisodeId) {
        self.db.changes.change(Change::Pending);

        if let Some(e) = self.db.episodes.get(episode_id) {
            self.db.pending.remove(e.series());
        }
    }

    /// Remove all watches of the given episode.
    pub(crate) fn remove_episode_watch(&mut self, episode_id: &EpisodeId, watch_id: &Uuid) {
        tracing::trace!(?episode_id, ?watch_id,);

        let (Some(w), Some(episode)) = (self.db.watched.remove_watch(watch_id), self.db.episodes.get(episode_id)) else {
            return;
        };

        self.db.changes.change(Change::Watched);
        self.db.changes.change(Change::Pending);

        self.db.pending.extend([Pending {
            series: *episode.series(),
            episode: episode.id,
            timestamp: w.timestamp,
        }]);
    }

    /// Remove all watches of the given episode.
    pub(crate) fn remove_season_watches(
        &mut self,
        now: &DateTime<Utc>,
        series_id: &SeriesId,
        season: &SeasonNumber,
    ) {
        let mut removed = 0;

        for e in self.db.episodes.by_series(series_id) {
            if e.season == *season {
                removed += self.db.watched.remove_by_episode(&e.id);
            }
        }

        if removed > 0 {
            self.db.changes.change(Change::Watched);
        }

        if self.db.pending.remove(series_id).is_some() {
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
                series: *series_id,
                episode: e.id,
                timestamp: pending_timestamp(now, [timestamp, e.aired_timestamp()]),
            }]);

            self.db.changes.change(Change::Pending);
        }
    }

    /// Save changes made.
    pub(crate) fn save_changes(&mut self) -> impl Future<Output = Result<()>> {
        if self.db.changes.contains(Change::Series) || self.db.changes.contains(Change::Schedule) {
            self.build_schedule();
        }

        self.db.save_changes(&self.paths, self.do_not_save)
    }

    /// Remove pending for the given series.
    fn remove_pending(&mut self, series_id: &SeriesId) {
        if self.db.pending.remove(series_id).is_some() {
            self.db.changes.change(Change::Pending);
        }
    }

    /// Populate pending from a series where we don't know which episode to
    /// populate from.
    #[tracing::instrument(skip(self))]
    pub(crate) fn populate_pending(&mut self, now: &DateTime<Utc>, id: &SeriesId) {
        if self.db.pending.get(id).is_some() {
            // Do nothing since we already have a pending episode.
            return;
        }

        let last = self.db.watched.by_series(id).next_back();

        let mut cur = if let Some(WatchedKind::Series { episode, .. }) = last.map(|w| &w.kind) {
            tracing::trace!(?episode, "episode after watched");
            self.db.episodes.get(episode).and_then(EpisodeRef::next)
        } else {
            tracing::trace!("finding next unwatched episode");
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

        tracing::trace!(episode = ?e.id, "set as pending");

        self.db.changes.change(Change::Pending);
        // Mark the next episode in the show as pending.
        self.db.pending.extend([Pending {
            series: *id,
            episode: e.id,
            timestamp: pending_timestamp(now, [last.map(|w| w.timestamp), e.aired_timestamp()]),
        }]);
    }

    /// Populate pending from a known episode ID.
    fn populate_pending_from(&mut self, now: &DateTime<Utc>, series_id: &SeriesId, id: &EpisodeId) {
        let Some(e) = self.db.episodes.get(id).and_then(|e| e.next()) else {
            return;
        };

        // Mark the next episode in the show as pending.
        self.db.changes.change(Change::Pending);

        let timestamp = e.aired_timestamp().map(|t| t.max(*now)).unwrap_or(*now);

        self.db.pending.extend([Pending {
            series: *series_id,
            episode: e.id,
            timestamp,
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
    pub(crate) fn theme(&self) -> &Theme {
        &self.current_theme
    }

    /// Set the theme configuration option.
    pub(crate) fn set_theme(&mut self, theme: ThemeType) {
        self.db.config.theme = theme;
        self.db.changes.change(Change::Config);
        self.current_theme = self.db.config.theme();
    }

    /// Set the theme configuration option.
    pub(crate) fn set_tvdb_legacy_api_key(&mut self, api_key: String) {
        self.tvdb.set_api_key(&api_key);
        self.db.config.tvdb_legacy_apikey = api_key;
        self.db.changes.change(Change::Config);
    }

    /// Set the theme configuration option.
    pub(crate) fn set_tmdb_api_key(&mut self, api_key: String) {
        self.tmdb.set_api_key(&api_key);
        self.db.config.tmdb_api_key = api_key;
        self.db.changes.change(Change::Config);
    }

    /// Check if series is tracked.
    pub(crate) fn get_series_by_remote(&self, id: &RemoteSeriesId) -> Option<&Series> {
        let id = self.db.remotes.get_series(id)?;
        self.db.series.get(&id)
    }

    /// Check if movie is tracked.
    pub(crate) fn get_movie_by_remote(&self, _: &RemoteMovieId) -> Option<&Movie> {
        // TODO: implement this.
        None
    }

    /// Remove the given series by ID.
    pub(crate) fn remove_series(&mut self, series_id: &SeriesId) {
        let _ = self.db.series.remove(series_id);
        let _ = self.db.episodes.remove(series_id);
        let _ = self.db.seasons.remove(series_id);
        self.db.changes.change(Change::Queue);
        self.db.changes.remove_series(series_id);

        if self.db.tasks.remove_tasks_by(|t| t.is_series(series_id)) != 0 {
            self.db.changes.change(Change::Queue);
        }
    }

    /// Download series using a remote identifier.
    #[tracing::instrument(skip(self))]
    pub(crate) fn download_series(
        &self,
        remote_id: &RemoteSeriesId,
        if_none_match: Option<&Etag>,
        series_id: Option<&SeriesId>,
    ) -> impl Future<Output = Result<Option<NewSeries>>> {
        let tvdb = self.tvdb.clone();
        let tmdb = self.tmdb.clone();
        let proxy = self.db.remotes.proxy();
        let remote_id = *remote_id;
        let if_none_match = if_none_match.cloned();
        let series_id = series_id.copied();

        async move {
            let lookup_series = |q| {
                if let Some(series_id) = series_id {
                    return Some(series_id);
                }

                proxy.find_series_by_remote(q)
            };

            let lookup_episode = |q| proxy.find_episode_by_remote(q);

            let data = match remote_id {
                RemoteSeriesId::Tvdb { id } => {
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
                RemoteSeriesId::Tmdb { id } => {
                    let Some((series, remote_ids, last_etag, last_modified, seasons)) = tmdb.series(id, lookup_series, if_none_match.as_ref()).await? else {
                        tracing::trace!("{remote_id}: not changed");
                        return Ok(None);
                    };

                    let mut episodes = Vec::new();

                    for season in &seasons {
                        let new_episodes = tmdb
                            .download_episodes(id, season.number, &lookup_episode)
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
                RemoteSeriesId::Imdb { .. } => {
                    bail!("cannot download series data from imdb")
                }
            };

            Ok::<_, Error>(Some(data))
        }
    }

    /// If the series is already loaded in the local database, simply mark it as tracked.
    pub(crate) fn set_series_tracked_by_remote(&mut self, id: &RemoteSeriesId) -> bool {
        let Some(id) = self.db.remotes.get_series(id) else {
            return false;
        };

        self.track(&id)
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

    /// Insert a new tracked song.
    #[tracing::instrument(skip(self))]
    pub(crate) fn insert_new_series(&mut self, now: &DateTime<Utc>, data: NewSeries) {
        let series_id = data.series.id;

        for &remote_id in &data.remote_ids {
            if self.db.remotes.insert_series(remote_id, series_id) {
                self.db.changes.change(Change::Remotes);
            }
        }

        if let Some(remote_id) = &data.series.remote_id {
            if let Some(etag) = data.last_etag {
                if self.db.sync.update_last_etag(&series_id, remote_id, etag) {
                    self.db.changes.change(Change::Sync);
                }
            }

            if let Some(last_modified) = &data.last_modified {
                if self
                    .db
                    .sync
                    .update_last_modified(&series_id, remote_id, Some(&last_modified))
                {
                    self.db.changes.change(Change::Sync);
                }
            }
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
            self.db.series.insert(data.series);
        }

        // Remove any pending episodes for the given series.
        self.populate_pending(now, &series_id);
        self.db.changes.add_series(&series_id);
    }

    /// Ensure that a collection of the given image ids are loaded.
    pub(crate) fn load_images(
        &self,
        images: Vec<(ImageKey, ImageV2)>,
    ) -> impl Future<Output = Result<Vec<(ImageKey, Handle)>>> {
        use futures::StreamExt;

        let paths = self.paths.clone();
        let tvdb = self.tvdb.clone();
        let tmdb = self.tmdb.clone();

        async move {
            let mut output = Vec::with_capacity(images.len());
            let mut futures = FuturesUnordered::new();

            for (key, image) in images {
                let paths = paths.clone();
                let tvdb = tvdb.clone();
                let tmdb = tmdb.clone();

                futures.push(async move {
                    let hash = image.hash();

                    let handle = match &image {
                        ImageV2::Tvdb { uri } => {
                            cache::image(&paths.images, &tvdb, uri.as_ref(), hash, key.hint).await
                        }
                        ImageV2::Tmdb { uri } => {
                            cache::image(&paths.images, &tmdb, uri.as_ref(), hash, key.hint).await
                        }
                    };

                    let handle = handle.with_context(|| anyhow!("downloading: {image:?}"))?;
                    Ok::<_, Error>((key, handle))
                });
            }

            while let Some(result) = futures.next().await {
                output.push(result?);
            }

            Ok(output)
        }
    }

    /// Prevents the service from saving anything to the filesystem.
    pub fn do_not_save(&mut self) {
        self.do_not_save = true;
    }

    /// Get existing id by remote if it exists.
    pub(crate) fn existing_by_remote_ids<I>(&self, ids: I) -> Option<SeriesId>
    where
        I: IntoIterator<Item = RemoteSeriesId>,
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
        timestamp: DateTime<Utc>,
    ) {
        self.db.watched.insert(Watched {
            id: Uuid::new_v4(),
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
        async move { tvdb.search_by_name(&query).await }
    }

    /// Search series from tmdb.
    pub(crate) fn search_series_tmdb(
        &self,
        query: &str,
    ) -> impl Future<Output = Result<Vec<SearchSeries>>> {
        let tmdb = self.tmdb.clone();
        let query = query.to_owned();
        async move { tmdb.search_series(&query).await }
    }

    /// Search movies from tmdb.
    pub(crate) fn search_movies_tmdb(
        &self,
        query: &str,
    ) -> impl Future<Output = Result<Vec<SearchMovie>>> {
        let tmdb = self.tmdb.clone();
        let query = query.to_owned();
        async move { tmdb.search_movies(&query).await }
    }

    /// Build schedule information.
    pub(crate) fn build_schedule(&mut self) {
        let mut current = self.now;

        let mut days = Vec::new();

        while current
            .signed_duration_since(self.now)
            .num_days()
            .unsigned_abs()
            <= self.config().schedule_duration_days
        {
            let mut schedule = Vec::new();

            for series in self.db.series.iter() {
                if !series.tracked {
                    continue;
                }

                let mut scheduled_episodes = Vec::new();

                for e in self.episodes(&series.id) {
                    let Some(air_date) = &e.aired else {
                        continue;
                    };

                    if *air_date != current {
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
                    date: current,
                    schedule,
                });
            }

            let Some(next) = current.checked_add_days(Days::new(1)) else {
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
    pub(crate) fn next_task(
        &mut self,
        now: &DateTime<Utc>,
        timed_out: Option<Uuid>,
    ) -> Option<Task> {
        self.db.tasks.next_task(now, timed_out)
    }

    /// Next duration to sleep.
    #[inline]
    pub(crate) fn next_task_sleep(&self, now: &DateTime<Utc>) -> Option<(u64, Uuid)> {
        self.db.tasks.next_sleep(now)
    }

    /// Check if the given task is pending.
    #[inline]
    pub(crate) fn task_status(&self, id: TaskId) -> Option<TaskStatus> {
        self.db.tasks.status(id)
    }

    /// Check if the given task is pending.
    #[inline]
    pub(crate) fn task_status_any(
        &self,
        ids: impl IntoIterator<Item = TaskId>,
    ) -> Option<TaskStatus> {
        ids.into_iter()
            .flat_map(|id| self.db.tasks.status(id))
            .next()
    }

    /// Mark task as completed.
    #[inline]
    pub(crate) fn complete_task(&mut self, task: Task) -> Option<TaskStatus> {
        match &task.kind {
            TaskKind::DownloadSeries {
                series_id,
                remote_id,
                last_modified,
                ..
            } => {
                let now = Utc::now();

                if self.db.sync.series_update_sync(
                    series_id,
                    remote_id,
                    &now,
                    last_modified.as_ref(),
                ) {
                    self.db.changes.change(Change::Sync);
                }
            }
            _ => {}
        }

        self.db.tasks.complete(&task)
    }

    /// Get remotes by series.
    pub(crate) fn remotes_by_series(
        &self,
        series_id: &SeriesId,
    ) -> impl ExactSizeIterator<Item = RemoteSeriesId> + '_ {
        self.db.remotes.get_by_series(series_id)
    }

    /// Clear last sync.
    pub(crate) fn clear_last_sync(&mut self) {
        for s in self.db.series.iter() {
            if let Some(remote_id) = &s.remote_id {
                if self.db.sync.clear_last_sync(&s.id, remote_id) {
                    self.db.changes.change(Change::Sync);
                }
            }
        }
    }

    /// Get last etag for the given series id.
    pub(crate) fn last_etag(&self, id: &SeriesId, remote_id: &RemoteSeriesId) -> Option<&Etag> {
        self.db.sync.last_etag(id, remote_id)
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
fn pending_timestamp<const N: usize>(
    now: &DateTime<Utc>,
    candidates: [Option<DateTime<Utc>>; N],
) -> DateTime<Utc> {
    if let Some(timestamp) = candidates.into_iter().flatten().max() {
        timestamp
    } else {
        *now
    }
}

pub(crate) enum RemainingSeason {
    /// Timestamp should be right now, but only if an episode has aired.
    Aired,
    /// Timestamp should be the air date of the episode.
    AirDate,
}
