use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt;
use std::future::Future;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::{anyhow, bail, Context, Error, Result};
use chrono::{DateTime, Days, Local, NaiveDate, Utc};
use futures::stream::FuturesUnordered;
use iced::Theme;
use iced_native::image::Handle;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::api::themoviedb;
use crate::api::thetvdb;
use crate::assets::ImageKey;
use crate::cache::{self};
use crate::model::{
    Config, Episode, EpisodeId, Etag, Image, RemoteEpisodeId, RemoteId, RemoteSeriesId,
    ScheduledDay, ScheduledSeries, SearchSeries, Season, SeasonNumber, Series, SeriesId, Task,
    TaskFinished, TaskKind, ThemeType, Watched,
};
use crate::queue::{Queue, TaskRunning, TaskStatus};

/// Data encapsulating a newly added series.
#[derive(Clone)]
pub(crate) struct NewSeries {
    series: Series,
    episodes: Vec<Episode>,
    seasons: Vec<Season>,
    refresh_pending: bool,
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
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub(crate) struct Pending {
    pub(crate) series: SeriesId,
    pub(crate) episode: EpisodeId,
    pub(crate) timestamp: DateTime<Utc>,
}

/// A pending thing to watch.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PendingRef<'a> {
    pub(crate) series: &'a Series,
    pub(crate) season: Option<&'a Season>,
    pub(crate) episode: &'a Episode,
}

#[derive(Clone, Copy, fixed_map::Key)]
enum Change {
    // Configuration file has changed.
    Config,
    // Watched list has changed.
    Watched,
    // Pending list has changed.
    Pending,
    // Series list has changed.
    Series,
    // Remotes list has changed.
    Remotes,
    // Task queue has changed.
    Queue,
    // Schedule changed.
    Schedule,
}

#[derive(Default)]
struct Changes {
    // Set of changes to apply to database.
    set: fixed_map::Set<Change>,
    // Series removed.
    remove: HashSet<SeriesId>,
    // Series added.
    add: HashSet<SeriesId>,
}

impl Changes {
    #[inline]
    fn has_changes(&self) -> bool {
        !self.set.is_empty() || !self.remove.is_empty() || !self.add.is_empty()
    }
}

#[derive(Default)]
struct SeriesDatabase {
    data: Vec<Series>,
    by_id: HashMap<SeriesId, usize>,
}

impl SeriesDatabase {
    /// Get a series immutably.
    fn get(&self, id: &SeriesId) -> Option<&Series> {
        let &index = self.by_id.get(id)?;
        self.data.get(index)
    }

    /// Get a series mutably.
    fn get_mut(&mut self, id: &SeriesId) -> Option<&mut Series> {
        let &index = self.by_id.get(id)?;
        self.data.get_mut(index)
    }

    /// Remove the series by the given identifier.
    fn remove(&mut self, id: &SeriesId) -> Option<Series> {
        let index = self.by_id.remove(id)?;
        let value = self.data.swap_remove(index);
        let data = &mut self.data[index..];

        data.sort_by(|a, b| a.title.cmp(&b.title));

        for (n, s) in data.iter().enumerate() {
            self.by_id.insert(s.id, index + n);
        }

        Some(value)
    }

    /// Insert the given series.
    fn push(&mut self, series: Series) {
        self.data.push(series);
        self.data.sort_by(|a, b| a.title.cmp(&b.title));
        self.by_id.clear();

        for (index, s) in self.data.iter().enumerate() {
            self.by_id.insert(s.id, index);
        }
    }
}

#[derive(Default)]
struct Database {
    /// Application configuration.
    config: Config,
    /// Remote series.
    remote_series: BTreeMap<RemoteSeriesId, SeriesId>,
    /// Episode IDs to remotes.
    remote_series_rev: HashMap<SeriesId, BTreeSet<RemoteSeriesId>>,
    /// Remote episodes.
    remote_episodes: BTreeMap<RemoteEpisodeId, EpisodeId>,
    /// Episode IDs to remotes.
    remote_episodes_rev: HashMap<EpisodeId, BTreeSet<RemoteEpisodeId>>,
    /// Series database.
    series: SeriesDatabase,
    /// Episodes collection.
    episodes: HashMap<SeriesId, Vec<Episode>>,
    /// Seasons collection.
    seasons: HashMap<SeriesId, Vec<Season>>,
    /// Episode to watch history.
    watched: BTreeMap<EpisodeId, Vec<Watched>>,
    /// Ordered list of things to watch.
    pending: Vec<Pending>,
    /// Keeping track of changes to be saved.
    changes: Changes,
    /// Download queue.
    tasks: Queue,
}

struct Paths {
    lock: tokio::sync::Mutex<()>,
    config: Box<Path>,
    remotes: Box<Path>,
    queue: Box<Path>,
    images: Box<Path>,
    series: Box<Path>,
    watched: Box<Path>,
    pending: Box<Path>,
    episodes: Box<Path>,
    seasons: Box<Path>,
}

/// Background service taking care of all state handling.
pub struct Service {
    paths: Arc<Paths>,
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
    pub fn new() -> Result<Self> {
        let dirs = directories_next::ProjectDirs::from("se.tedro", "setbac", "OnTV")
            .context("missing project dirs")?;

        let paths = Paths {
            lock: tokio::sync::Mutex::new(()),
            config: dirs.config_dir().join("config.json").into(),
            remotes: dirs.config_dir().join("remotes.json").into(),
            queue: dirs.config_dir().join("queue.json").into(),
            series: dirs.config_dir().join("series.json").into(),
            watched: dirs.config_dir().join("watched.json").into(),
            pending: dirs.config_dir().join("pending.json").into(),
            episodes: dirs.config_dir().join("episodes").into(),
            seasons: dirs.config_dir().join("seasons").into(),
            images: dirs.cache_dir().join("images").into(),
        };

        let db = load_database(&paths)?;
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
    pub(crate) fn now(&self) -> NaiveDate {
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

    /// Get a single series mutably.
    pub(crate) fn series_mut(&mut self, id: &SeriesId) -> Option<&mut Series> {
        let series = self.db.series.get_mut(id)?;
        self.db.changes.set.insert(Change::Series);
        Some(series)
    }

    /// Get list of series.
    pub(crate) fn all_series(&self) -> &[Series] {
        &self.db.series.data
    }

    /// Get list of series mutably and mark all series as changed.
    pub(crate) fn all_series_mut(&mut self) -> &mut [Series] {
        self.db.changes.set.insert(Change::Series);
        &mut self.db.series.data
    }

    /// Iterator over available episodes.
    #[inline]
    pub(crate) fn episodes(&self, id: &SeriesId) -> &[Episode] {
        let Some(values) = self.db.episodes.get(id) else {
            return &[];
        };

        values
    }

    /// Iterator over available seasons.
    #[inline]
    pub(crate) fn seasons(&self, series_id: &SeriesId) -> &[Season] {
        self.db
            .seasons
            .get(series_id)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    /// Get all the watches for the given episode.
    #[inline]
    pub(crate) fn watched(&self, episode_id: &EpisodeId) -> &[Watched] {
        self.db
            .watched
            .get(episode_id)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    /// Get task queue.
    pub(crate) fn tasks(&self) -> impl ExactSizeIterator<Item = &Task> {
        self.db.tasks.pending()
    }

    /// Get task queue.
    pub(crate) fn running_tasks(&self) -> impl ExactSizeIterator<Item = &TaskRunning> {
        self.db.tasks.running()
    }

    /// Test if episode is watched.
    pub(crate) fn watch_count(&self, episode_id: &EpisodeId) -> usize {
        self.db
            .watched
            .get(episode_id)
            .map(Vec::len)
            .unwrap_or_default()
    }

    /// Get season summary statistics.
    pub(crate) fn season_watched(
        &self,
        series_id: &SeriesId,
        season: &SeasonNumber,
    ) -> (usize, usize) {
        let mut total = 0;
        let mut watched = 0;

        for episode in self
            .episodes(series_id)
            .iter()
            .filter(|e| e.season == *season)
        {
            total += 1;
            watched += usize::from(self.watch_count(&episode.id) != 0);
        }

        (watched, total)
    }

    /// Get the pending episode for the given series.
    pub(crate) fn get_pending(&self, series_id: &SeriesId) -> Option<&Pending> {
        self.db.pending.iter().find(|p| p.series == *series_id)
    }

    /// Return list of pending episodes.
    pub(crate) fn pending(&self) -> impl DoubleEndedIterator<Item = PendingRef<'_>> {
        self.db.pending.iter().flat_map(|p| {
            let series = self.db.series.get(&p.series)?;

            let episode = self
                .episodes(&p.series)
                .iter()
                .find(|e| e.id == p.episode)?;

            let season = self
                .seasons(&p.series)
                .iter()
                .find(|s| s.number == episode.season);

            Some(PendingRef {
                series,
                season,
                episode,
            })
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

        for s in &mut self.db.series.data {
            // Ignore series which are no longer tracked.
            if !s.tracked {
                continue;
            }

            let Some(remote_id) = s.remote_id else {
                continue;
            };

            // Reduce the number of API requests by ensuring we don't check for
            // updates more than each CACHE_TIME interval.
            if let Some(last_sync) = s.last_sync.get(&remote_id) {
                if now.signed_duration_since(*last_sync).num_seconds() < CACHE_TIME {
                    continue;
                }
            }

            let kind = match remote_id {
                RemoteSeriesId::Tvdb { .. } => TaskKind::CheckForUpdates {
                    series_id: s.id,
                    remote_id,
                },
                RemoteSeriesId::Tmdb { .. } => TaskKind::DownloadSeriesById { series_id: s.id },
                RemoteSeriesId::Imdb { .. } => continue,
            };

            let finished = TaskFinished::SeriesSynced {
                series_id: s.id,
                remote_id,
                last_modified: None,
            };

            if self.db.tasks.push(kind, Some(finished)) {
                self.db.changes.set.insert(Change::Series);
                self.db.changes.set.insert(Change::Queue);
            }
        }
    }

    /// Check for update for the given series.
    pub(crate) fn check_for_updates(
        &mut self,
        series_id: &SeriesId,
        remote_id: &RemoteSeriesId,
    ) -> Option<impl Future<Output = Result<Option<(TaskKind, TaskFinished)>>>> {
        let Some(s) = self.db.series.get(series_id) else {
            return None;
        };

        match s.remote_id {
            Some(RemoteSeriesId::Tmdb { .. }) => {
                let kind = TaskKind::DownloadSeriesById { series_id: s.id };

                if self.db.tasks.push(kind, None) {
                    self.db.changes.set.insert(Change::Queue);
                }

                return None;
            }
            _ => {}
        }

        let last_modified = s.last_modified;
        let tvdb = self.tvdb.clone();

        let series_id = s.id;
        let remote_id = *remote_id;

        Some(async move {
            let finished = match remote_id {
                RemoteSeriesId::Tvdb { id } => {
                    let Some(update) = tvdb.series_last_modified(id).await? else {
                        bail!("{series_id}/{remote_id}: missing last-modified in api");
                    };

                    log::trace!(
                        "{series_id}/{remote_id}: last modified {update:?} (existing {last_modified:?})"
                    );

                    if matches!(last_modified, Some(last_modified) if last_modified >= update) {
                        return Ok(None);
                    }

                    TaskFinished::SeriesSynced {
                        series_id,
                        remote_id,
                        last_modified: Some(update),
                    }
                }
                // Nothing to do with the IMDB remote.
                remote_id => bail!("{remote_id}: not supported for checking for updates"),
            };

            let kind = TaskKind::DownloadSeriesById { series_id };

            Ok(Some((kind, finished)))
        })
    }

    /// Push a single task to the queue.
    pub(crate) fn push_task(&mut self, kind: TaskKind, finished: Option<TaskFinished>) -> bool {
        if self.db.tasks.push(kind, finished) {
            self.db.changes.set.insert(Change::Queue);
            true
        } else {
            false
        }
    }

    /// Add updates to download to the queue.
    pub(crate) fn push_tasks<I>(&mut self, it: I)
    where
        I: IntoIterator<Item = (TaskKind, Option<TaskFinished>)>,
    {
        let mut any = false;

        for (kind, finished) in it {
            any |= self.db.tasks.push(kind, finished);
        }

        if any {
            self.db.changes.set.insert(Change::Queue);
        }
    }

    /// Mark an episode as watched at the given timestamp.
    pub(crate) fn watch_remaining_season(
        &mut self,
        series_id: &SeriesId,
        season: &SeasonNumber,
        timestamp: DateTime<Utc>,
    ) {
        let mut last = None;

        for episode in self
            .db
            .episodes
            .get(series_id)
            .into_iter()
            .flatten()
            .filter(|e| e.season == *season)
        {
            if self.watch_count(&episode.id) > 0 {
                continue;
            }

            if !episode.has_aired(&timestamp) {
                continue;
            }

            self.db
                .watched
                .entry(episode.id)
                .or_default()
                .push(Watched {
                    id: Uuid::new_v4(),
                    series: *series_id,
                    episode: episode.id,
                    timestamp,
                });

            self.db.changes.set.insert(Change::Watched);
            last = Some(episode.id);
        }

        let Some(last) = last else {
            return;
        };

        self.setup_pending(series_id, Some(&last));
    }

    /// Mark an episode as watched at the given timestamp.
    pub(crate) fn watch(
        &mut self,
        series_id: &SeriesId,
        episode_id: &EpisodeId,
        timestamp: DateTime<Utc>,
    ) {
        self.db
            .watched
            .entry(*episode_id)
            .or_default()
            .push(Watched {
                id: Uuid::new_v4(),
                series: *series_id,
                episode: *episode_id,
                timestamp,
            });

        self.db.changes.set.insert(Change::Watched);
        self.setup_pending(series_id, Some(episode_id))
    }

    /// Skip an episode.
    pub(crate) fn skip(
        &mut self,
        series_id: &SeriesId,
        episode_id: &EpisodeId,
        now: DateTime<Utc>,
    ) {
        let Some(episodes) = self.db.episodes.get(series_id) else {
            return;
        };

        let mut it = episodes.iter();

        for episode in it.by_ref() {
            if episode.id == *episode_id {
                break;
            }
        }

        self.db.changes.set.insert(Change::Pending);

        let Some(episode) = it.find(|e| e.has_aired(&now)) else {
            self.db.pending.retain(|p| p.series != *series_id);
            return;
        };

        for pending in self
            .db
            .pending
            .iter_mut()
            .filter(|p| p.episode == *episode_id)
        {
            pending.episode = episode.id;
            pending.timestamp = now;
        }
    }

    /// Select the next pending episode to use for a show.
    pub(crate) fn select_pending(
        &mut self,
        series_id: &SeriesId,
        episode_id: &EpisodeId,
        now: DateTime<Utc>,
    ) {
        self.db.changes.set.insert(Change::Pending);

        // Try to modify in-place.
        if let Some(pending) = self.db.pending.iter_mut().find(|p| p.series == *series_id) {
            pending.episode = *episode_id;
            pending.timestamp = now;
        } else {
            self.db.pending.push(Pending {
                series: *series_id,
                episode: *episode_id,
                timestamp: now,
            });
        }

        self.db
            .pending
            .sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
    }

    /// Remove all watches of the given episode.
    pub(crate) fn remove_episode_watch(
        &mut self,
        series_id: &SeriesId,
        episode_id: &EpisodeId,
        watch_id: &Uuid,
    ) {
        log::trace!(
            "remove series_id: {series_id}, episode_id: {episode_id}, watch_id: {watch_id}"
        );

        let watched = self.db.watched.entry(*episode_id).or_default();
        watched.retain(|w| w.id != *watch_id);
        self.db.changes.set.insert(Change::Watched);

        if watched.is_empty() {
            self.db.pending.retain(|p| p.series != *series_id);

            let last_timestamp = self
                .episodes(series_id)
                .iter()
                .take_while(|e| e.id != *episode_id)
                .flat_map(|e| self.watched(&e.id))
                .map(|w| w.timestamp)
                .max();

            self.db.pending.push(Pending {
                series: *series_id,
                episode: *episode_id,
                timestamp: last_timestamp.unwrap_or_else(Utc::now),
            });

            self.db.changes.set.insert(Change::Pending);
        }
    }

    /// Remove all watches of the given episode.
    pub(crate) fn remove_season_watches(
        &mut self,
        series_id: &SeriesId,
        season: &SeasonNumber,
        now: DateTime<Utc>,
    ) {
        let Some(episodes) = self.db.episodes.get(series_id) else {
            return;
        };

        let mut last_timestamp = None;

        for e in episodes {
            if e.season == *season {
                let _ = self.db.watched.remove(&e.id);
            } else if e.season < *season {
                last_timestamp = last_timestamp
                    .into_iter()
                    .chain(self.watched(&e.id).iter().map(|w| w.timestamp))
                    .max();
            }
        }

        self.db.changes.set.insert(Change::Watched);
        self.db.pending.retain(|p| p.series != *series_id);
        self.db.changes.set.insert(Change::Pending);

        let Some(episodes) = self.db.episodes.get(series_id) else {
            return;
        };

        // Find the first episode matching the given season and make that the
        // pending episode.
        if let Some(episode) = episodes
            .iter()
            .find(|e| e.season == *season && e.has_aired(&now))
        {
            self.db.pending.push(Pending {
                series: *series_id,
                episode: episode.id,
                timestamp: last_timestamp.unwrap_or(now),
            });
        }
    }

    /// Set up next pending episode.
    fn setup_pending(&mut self, series_id: &SeriesId, episode_id: Option<&EpisodeId>) {
        let now = Utc::now();
        self.populate_pending(&now, series_id, episode_id);
    }

    /// Save changes made.
    pub(crate) fn save_changes(&mut self) -> impl Future<Output = Result<()>> {
        let changes = std::mem::take(&mut self.db.changes);

        if changes.set.contains(Change::Series) || changes.set.contains(Change::Schedule) {
            self.build_schedule();
        }

        let config = changes
            .set
            .contains(Change::Config)
            .then(|| self.db.config.clone());

        let watched = changes
            .set
            .contains(Change::Watched)
            .then(|| self.db.watched.clone().into_iter().flat_map(|(_, v)| v));

        let pending = changes
            .set
            .contains(Change::Pending)
            .then(|| self.db.pending.clone());

        let series = changes
            .set
            .contains(Change::Series)
            .then(|| self.db.series.data.clone());

        let queue = changes
            .set
            .contains(Change::Queue)
            .then(|| self.db.tasks.pending().cloned().collect::<Vec<_>>());

        let remove_series = changes.remove;
        let mut add_series = Vec::with_capacity(changes.add.len());

        for id in changes.add {
            let Some(episodes) = self.db.episodes.get(&id) else {
                continue;
            };

            let Some(seasons) = self.db.seasons.get(&id) else {
                continue;
            };

            add_series.push((id, episodes.clone(), seasons.clone()));
        }

        let remotes = if changes.set.contains(Change::Remotes) {
            let mut output = Vec::with_capacity(
                self.db.remote_series_rev.len() + self.db.remote_episodes_rev.len(),
            );

            for (&uuid, remotes) in &self.db.remote_series_rev {
                output.push(RemoteId::Series {
                    uuid,
                    remotes: remotes.clone(),
                });
            }

            for (&uuid, remotes) in &self.db.remote_episodes_rev {
                output.push(RemoteId::Episode {
                    uuid,
                    remotes: remotes.clone(),
                });
            }

            Some(output)
        } else {
            None
        };

        let paths = self.paths.clone();

        let do_not_save = self.do_not_save;

        async move {
            if do_not_save {
                return Ok(());
            }

            let guard = paths.lock.lock().await;

            if let Some(config) = config {
                save_pretty("config", &paths.config, config).await?;
            }

            if let Some(series) = series {
                save_array("series", &paths.series, series).await?;
            }

            if let Some(watched) = watched {
                save_array("watched", &paths.watched, watched).await?;
            }

            if let Some(pending) = pending {
                save_array("pending", &paths.pending, pending).await?;
            }

            if let Some(remotes) = remotes {
                save_array("remotes", &paths.remotes, remotes).await?;
            }

            if let Some(queue) = queue {
                save_array("queue", &paths.queue, queue).await?;
            }

            for series_id in remove_series {
                let episodes = paths.episodes.join(format!("{series_id}.json"));
                let seasons = paths.seasons.join(format!("{series_id}.json"));
                let a = remove_file("episodes", &episodes);
                let b = remove_file("episodes", &seasons);
                let _ = tokio::try_join!(a, b)?;
            }

            for (series_id, episodes, seasons) in add_series {
                let episodes_path = paths.episodes.join(format!("{series_id}.json"));
                let seasons_path = paths.seasons.join(format!("{series_id}.json"));
                let a = save_array("episodes", &episodes_path, episodes);
                let b = save_array("seasons", &seasons_path, seasons);
                let _ = tokio::try_join!(a, b)?;
            }

            drop(guard);
            Ok(())
        }
    }

    /// Ensure that at least one pending episode is present for the given
    /// series.
    pub(crate) fn populate_pending(
        &mut self,
        now: &DateTime<Utc>,
        series_id: &SeriesId,
        episode_id: Option<&EpisodeId>,
    ) {
        // Remove any pending episodes for the given series.
        self.db.pending.retain(|p| p.series != *series_id);
        self.db.changes.set.insert(Change::Pending);

        // Populate the next pending episode.
        let Some(episodes) = self.db.episodes.get(series_id) else {
            return;
        };

        let mut last_timestamp = None;

        let episode = if let Some(last) = episode_id {
            let mut it = episodes.iter();

            // Find the first episode which is after the last episode indicated.
            loop {
                let Some(e) = it.next() else {
                    break None;
                };

                if e.id == *last {
                    last_timestamp = self.watched(&e.id).iter().map(|w| w.timestamp).max();
                    break it.next();
                }
            }
        } else {
            let mut last = None;

            // Find the first episode which is *not* in our watch history.
            for episode in episodes {
                if self.watch_count(&episode.id) == 0 {
                    last = Some(last.unwrap_or(episode));
                    continue;
                }

                last_timestamp = self.watched(&episode.id).iter().map(|w| w.timestamp).max();

                last = None;
            }

            last
        };

        self.db.changes.set.insert(Change::Pending);

        // Mark the first episode (that has aired).
        if let Some(e) = episode.filter(|e| e.has_aired(now)) {
            // Mark the next episode in the show as pending.
            self.db.pending.push(Pending {
                series: *series_id,
                episode: e.id,
                timestamp: last_timestamp.unwrap_or(*now),
            });
        }

        self.db
            .pending
            .sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
    }

    /// Get current configuration.
    pub(crate) fn config(&self) -> &Config {
        &self.db.config
    }

    /// Get configuration mutably indicating that it has been changed.
    pub(crate) fn config_mut(&mut self) -> &mut Config {
        self.db.changes.set.insert(Change::Config);
        &mut self.db.config
    }

    /// Get the current theme.
    pub(crate) fn theme(&self) -> &Theme {
        &self.current_theme
    }

    /// Set the theme configuration option.
    pub(crate) fn set_theme(&mut self, theme: ThemeType) {
        self.db.config.theme = theme;
        self.db.changes.set.insert(Change::Config);
        self.current_theme = self.db.config.theme();
    }

    /// Set the theme configuration option.
    pub(crate) fn set_tvdb_legacy_api_key(&mut self, api_key: String) {
        self.tvdb.set_api_key(&api_key);
        self.db.config.tvdb_legacy_apikey = api_key;
        self.db.changes.set.insert(Change::Config);
    }

    /// Set the theme configuration option.
    pub(crate) fn set_tmdb_api_key(&mut self, api_key: String) {
        self.tmdb.set_api_key(&api_key);
        self.db.config.tmdb_api_key = api_key;
        self.db.changes.set.insert(Change::Config);
    }

    /// Check if series is tracked.
    pub(crate) fn get_series_by_remote(&self, id: RemoteSeriesId) -> Option<&Series> {
        let id = self.db.remote_series.get(&id)?;
        self.db.series.get(id)
    }

    /// Remove the given series by ID.
    pub(crate) fn remove_series(&mut self, series_id: &SeriesId) {
        let _ = self.db.series.remove(series_id);
        let _ = self.db.episodes.remove(series_id);
        let _ = self.db.seasons.remove(series_id);
        self.db.changes.set.insert(Change::Series);
        self.db.changes.set.insert(Change::Queue);
        self.db.changes.add.remove(series_id);
        self.db.changes.remove.insert(*series_id);

        if self.db.tasks.remove_tasks_by(|t| t.is_series(series_id)) != 0 {
            self.db.changes.set.insert(Change::Queue);
        }
    }

    /// Enable tracking of the series with the given id.
    pub(crate) fn download_series_by_remote(
        &mut self,
        remote_id: &RemoteSeriesId,
        if_none_match: Option<&Etag>,
    ) -> impl Future<Output = Result<Option<NewSeries>>> {
        self.download_series(remote_id, true, if_none_match)
    }

    /// Download series using a remote identifier.
    pub(crate) fn download_series(
        &mut self,
        remote_id: &RemoteSeriesId,
        refresh_pending: bool,
        if_none_match: Option<&Etag>,
    ) -> impl Future<Output = Result<Option<NewSeries>>> {
        let tvdb = self.tvdb.clone();
        let tmdb = self.tmdb.clone();
        let series = self.db.remote_series.clone();
        let episodes = self.db.remote_episodes.clone();
        let episodes_remotes = self.db.remote_episodes_rev.clone();

        let lookup_series =
            move |q| Some(*series.iter().find(|(remote_id, _)| **remote_id == q)?.1);

        let remote_id = *remote_id;

        let if_none_match = if_none_match.cloned();

        async move {
            let lookup_episode =
                move |q| Some(*episodes.iter().find(|(remote_id, _)| **remote_id == q)?.1);

            let (series, seasons, episodes) = match remote_id {
                RemoteSeriesId::Tvdb { id } => {
                    let series = tvdb.series(id, lookup_series);
                    let episodes = tvdb.series_episodes(id, lookup_episode);
                    let (series, episodes) = tokio::try_join!(series, episodes)?;
                    let seasons = episodes_into_seasons(&episodes);
                    (series, seasons, episodes)
                }
                RemoteSeriesId::Tmdb { id } => {
                    let Some((series, seasons)) = tmdb.series(id, lookup_series, if_none_match.as_ref()).await? else {
                        log::trace!("{remote_id}: not changed");
                        return Ok(None);
                    };

                    let mut episodes = Vec::new();

                    for season in &seasons {
                        let new_episodes = tmdb
                            .episodes(id, season.number, &lookup_episode, |id| {
                                episodes_remotes.get(&id)
                            })
                            .await?;
                        episodes.extend(new_episodes);
                    }

                    (series, seasons, episodes)
                }
                RemoteSeriesId::Imdb { .. } => {
                    bail!("cannot download series data from imdb")
                }
            };

            let data = NewSeries {
                series,
                episodes,
                seasons,
                refresh_pending,
            };

            Ok::<_, Error>(Some(data))
        }
    }

    /// If the series is already loaded in the local database, simply mark it as tracked.
    pub(crate) fn set_tracked_by_remote(&mut self, id: &RemoteSeriesId) -> bool {
        let Some(&id) = self.db.remote_series.get(id) else {
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
        self.db.changes.set.insert(Change::Series);
        self.setup_pending(series_id, None);
        true
    }

    /// Disable tracking of the series with the given id.
    pub(crate) fn untrack(&mut self, series_id: &SeriesId) {
        if let Some(s) = self.db.series.get_mut(series_id) {
            s.tracked = false;
            self.db.changes.set.insert(Change::Series);
        }

        self.db.pending.retain(|p| p.series != *series_id);
        self.db.changes.set.insert(Change::Pending);
    }

    /// Insert a new tracked song.
    pub(crate) fn insert_new_series(&mut self, data: NewSeries) {
        let series_id = data.series.id;

        for &remote_id in &data.series.remote_ids {
            if !matches!(self.db.remote_series.insert(remote_id, series_id), Some(id) if id == series_id)
            {
                self.db.changes.set.insert(Change::Remotes);
            }

            self.db
                .remote_series_rev
                .entry(series_id)
                .or_default()
                .insert(remote_id);
        }

        for episode in &data.episodes {
            for &remote_id in &episode.remote_ids {
                if !matches!(self.db.remote_episodes.insert(remote_id, episode.id), Some(id) if id == episode.id)
                {
                    self.db.changes.set.insert(Change::Remotes);
                }

                self.db
                    .remote_episodes_rev
                    .entry(episode.id)
                    .or_default()
                    .insert(remote_id);
            }
        }

        self.db.episodes.insert(series_id, data.episodes.clone());
        self.db.seasons.insert(series_id, data.seasons.clone());

        if let Some(current) = self.db.series.get_mut(&data.series.id) {
            *current = data.series;
        } else {
            self.db.series.push(data.series);
        }

        if data.refresh_pending {
            // Remove any pending episodes for the given series.
            let now = Utc::now();
            self.populate_pending(&now, &series_id, None);
        }

        self.db.changes.set.insert(Change::Series);
        self.db.changes.remove.remove(&series_id);
        self.db.changes.add.insert(series_id);
    }

    /// Ensure that a collection of the given image ids are loaded.
    pub(crate) fn load_images(
        &self,
        ids: &[ImageKey],
    ) -> impl Future<Output = Result<Vec<(ImageKey, Handle)>>> {
        use futures::StreamExt;

        let paths = self.paths.clone();
        let tvdb = self.tvdb.clone();
        let tmdb = self.tmdb.clone();
        let ids = ids.to_vec();

        async move {
            let mut output = Vec::with_capacity(ids.len());
            let mut futures = FuturesUnordered::new();

            for key in ids {
                let paths = paths.clone();
                let tvdb = tvdb.clone();
                let tmdb = tmdb.clone();

                futures.push(async move {
                    let handle = match key.id {
                        Image::Tvdb(id) => cache::image(&paths.images, &tvdb, id, key.hint).await?,
                        Image::Tmdb(id) => cache::image(&paths.images, &tmdb, id, key.hint).await?,
                    };

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
    pub(crate) fn existing_by_remote_ids<I>(&self, ids: I) -> Option<&SeriesId>
    where
        I: IntoIterator<Item = RemoteSeriesId>,
    {
        for remote_id in ids {
            if let Some(id) = self.db.remote_series.get(&remote_id) {
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
        self.db
            .watched
            .entry(episode_id)
            .or_default()
            .push(Watched {
                id: Uuid::new_v4(),
                series: series_id,
                episode: episode_id,
                timestamp,
            });

        self.db.changes.set.insert(Change::Watched);
    }

    /// Remove watch history matching the given series.
    pub(crate) fn clear_watches(&mut self, series_id: SeriesId) {
        for values in self.db.watched.values_mut() {
            values.retain(|w| w.series != series_id);
        }

        self.db.changes.set.insert(Change::Watched);
    }

    /// Find an episode using the given predicate.
    pub(crate) fn find_episode_by<P>(
        &self,
        series_id: &SeriesId,
        mut predicate: P,
    ) -> Option<&Episode>
    where
        P: FnMut(&Episode) -> bool,
    {
        self.episodes(series_id).iter().find(move |&e| predicate(e))
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

    /// Search tmdb.
    pub(crate) fn search_tmdb(
        &self,
        query: &str,
    ) -> impl Future<Output = Result<Vec<SearchSeries>>> {
        let tmdb = self.tmdb.clone();
        let query = query.to_owned();
        async move { tmdb.search_series(&query).await }
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

            for series in self.all_series() {
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
    pub(crate) fn task_status(&self, kind: &TaskKind) -> Option<TaskStatus> {
        self.db.tasks.status(kind)
    }

    /// Mark task as completed.
    #[inline]
    pub(crate) fn complete_task(&mut self, task: Task) -> Option<TaskStatus> {
        match &task.finished {
            Some(TaskFinished::SeriesSynced {
                series_id,
                remote_id,
                last_modified,
            }) => {
                if let Some(s) = self.series_mut(&series_id) {
                    if let Some(last_modified) = last_modified {
                        s.last_modified = Some(last_modified.clone());
                    }

                    s.last_sync.insert(*remote_id, Utc::now());
                }
            }
            None => {}
        }

        self.db.tasks.complete(&task)
    }
}

/// Load configuration file.
pub(crate) fn load_config(path: &Path) -> Result<Option<Config>> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };

    Ok(serde_json::from_slice(&bytes)?)
}

/// Try to load initial state.
fn load_database(paths: &Paths) -> Result<Database> {
    let mut db = Database::default();

    if let Some(config) = load_config(&paths.config)? {
        db.config = config;
    }

    if let Some(remotes) = load_array::<RemoteId>(&paths.remotes)? {
        for remote_id in remotes {
            match remote_id {
                RemoteId::Series { uuid, remotes } => {
                    for remote_id in remotes {
                        db.remote_series.insert(remote_id, uuid);
                        db.remote_series_rev
                            .entry(uuid)
                            .or_default()
                            .insert(remote_id);
                    }
                }
                RemoteId::Episode { uuid, remotes } => {
                    for remote_id in remotes {
                        db.remote_episodes.insert(remote_id, uuid);
                        db.remote_episodes_rev
                            .entry(uuid)
                            .or_default()
                            .insert(remote_id);
                    }
                }
            }
        }
    }

    if let Some(tasks) = load_array::<Task>(&paths.queue)? {
        for task in tasks {
            db.tasks.push_raw(task);
        }

        db.tasks.sort();
    }

    if let Some(series) = load_series(&paths.series)? {
        for s in series {
            for &id in &s.remote_ids {
                db.remote_series.insert(id, s.id);
            }

            let len = db.series.data.len();
            db.series.by_id.insert(s.id, len);
            db.series.data.push(s);
        }
    }

    if let Some(watched) = load_array::<Watched>(&paths.watched)? {
        for w in watched {
            db.watched.entry(w.episode).or_default().push(w);
        }
    }

    if let Some(pending) = load_array(&paths.pending)? {
        db.pending = pending;
    }

    if let Some(episodes) = load_directory(&paths.episodes)? {
        for (id, episodes) in episodes {
            db.episodes.insert(id, episodes);
        }
    }

    if let Some(seasons) = load_directory(&paths.seasons)? {
        for (id, seasons) in seasons {
            db.seasons.insert(id, seasons);
        }
    }

    Ok(db)
}

/// Load series from the given path.
fn load_series(path: &Path) -> Result<Option<Vec<Series>>> {
    let f = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };

    Ok(Some(
        load_array_from_reader(f).with_context(|| anyhow!("{}", path.display()))?,
    ))
}

/// Remove the given file.
async fn remove_file(what: &'static str, path: &Path) -> Result<()> {
    log::trace!("{what}: removing: {}", path.display());
    let _ = tokio::fs::remove_file(path).await;
    Ok(())
}

/// Save series to the given path.
fn save_pretty<I>(what: &'static str, path: &Path, data: I) -> impl Future<Output = Result<()>>
where
    I: 'static + Send + Serialize,
{
    use std::fs;
    use std::io::Write;

    log::debug!("saving {what}: {}", path.display());

    let path = path.to_owned();

    let task = tokio::spawn(async move {
        let Some(dir) = path.parent() else {
            anyhow::bail!("{what}: missing parent directory: {}", path.display());
        };

        if !matches!(fs::metadata(dir), Ok(m) if m.is_dir()) {
            fs::create_dir_all(dir)?;
        }

        let mut f = tempfile::NamedTempFile::new_in(dir)?;

        log::trace!("writing {what}: {}", f.path().display());

        serde_json::to_writer_pretty(&mut f, &data)?;
        f.write_all(&[b'\n'])?;

        f.flush()?;
        let (_, temp_path) = f.keep()?;

        log::trace!(
            "rename {what}: {} -> {}",
            temp_path.display(),
            path.display()
        );

        fs::rename(temp_path, path)?;
        Ok(())
    });

    async move {
        let output: Result<()> = task.await?;
        output
    }
}

/// Save series to the given path.
fn save_array<I>(what: &'static str, path: &Path, data: I) -> impl Future<Output = Result<()>>
where
    I: 'static + Send + IntoIterator,
    I::Item: Serialize,
{
    use std::fs;
    use std::io::Write;

    log::trace!("saving {what}: {}", path.display());

    let path = path.to_owned();

    let task = tokio::spawn(async move {
        let Some(dir) = path.parent() else {
            anyhow::bail!("{what}: missing parent directory: {}", path.display());
        };

        if !matches!(fs::metadata(dir), Ok(m) if m.is_dir()) {
            fs::create_dir_all(dir)?;
        }

        let mut f = tempfile::NamedTempFile::new_in(dir)?;

        log::trace!("writing {what}: {}", f.path().display());

        for line in data {
            serde_json::to_writer(&mut f, &line)?;
            f.write_all(&[b'\n'])?;
        }

        f.flush()?;
        let (_, temp_path) = f.keep()?;

        log::trace!(
            "rename {what}: {} -> {}",
            temp_path.display(),
            path.display()
        );

        fs::rename(temp_path, path)?;
        Ok(())
    });

    async move {
        let output: Result<()> = task.await?;
        output
    }
}

/// Load all episodes found on the given paths.
fn load_directory<I, T>(path: &Path) -> Result<Option<Vec<(I, Vec<T>)>>>
where
    I: FromStr,
    I::Err: fmt::Display,
    T: DeserializeOwned,
{
    use std::fs;

    let d = match fs::read_dir(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };

    let mut output = Vec::new();

    for e in d {
        let e = e?;

        let m = e.metadata()?;

        if !m.is_file() {
            continue;
        }

        let path = e.path();

        if !matches!(path.extension().and_then(|e| e.to_str()), Some("json")) {
            continue;
        }

        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };

        let Ok(id) = stem.parse() else {
            continue;
        };

        let f = std::fs::File::open(&path)?;
        let array = load_array_from_reader(f).with_context(|| anyhow!("{}", path.display()))?;
        output.push((id, array));
    }

    Ok(Some(output))
}

/// Load a simple array from a file.
fn load_array<T>(path: &Path) -> Result<Option<Vec<T>>>
where
    T: DeserializeOwned,
{
    let f = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(Error::from(e)).with_context(|| anyhow!("{}", path.display())),
    };

    Ok(Some(
        load_array_from_reader(f).with_context(|| anyhow!("{}", path.display()))?,
    ))
}

/// Load an array from the given reader line-by-line.
fn load_array_from_reader<I, T>(input: I) -> Result<Vec<T>>
where
    I: std::io::Read,
    T: DeserializeOwned,
{
    use std::io::{BufRead, BufReader};

    let mut output = Vec::new();

    for line in BufReader::new(input).lines() {
        let line = line?;
        let line = line.trim();

        if line.starts_with('#') || line.is_empty() {
            continue;
        }

        output.push(serde_json::from_str(line)?);
    }

    Ok(output)
}

/// Helper to build seasons out of known episodes.
fn episodes_into_seasons(episodes: &[Episode]) -> Vec<Season> {
    let mut map = BTreeMap::new();

    for e in episodes {
        let season = map.entry(e.season).or_insert_with(|| Season {
            number: e.season,
            ..Season::default()
        });

        season.air_date = match (season.air_date, e.aired) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (Some(t), _) | (_, Some(t)) => Some(t),
            _ => None,
        };
    }

    map.into_values().collect()
}
