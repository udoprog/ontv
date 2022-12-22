use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt;
use std::future::Future;
use std::path::Path;
use std::sync::Arc;

use anyhow::{anyhow, bail, Context, Error, Result};
use chrono::{DateTime, Duration, Utc};
use iced_native::image::Handle;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::api::themoviedb;
use crate::api::thetvdb;
use crate::cache;
use crate::model::{
    Config, Episode, Image, RemoteEpisodeId, RemoteId, RemoteSeriesId, Season, SeasonNumber,
    Series, ThemeType, Watched,
};

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
    pub(crate) fn series_id(&self) -> Uuid {
        self.series.id
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
    pub(crate) series: Uuid,
    pub(crate) episode: Uuid,
    pub(crate) timestamp: DateTime<Utc>,
}

/// A pending thing to watch.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PendingRef<'a> {
    pub(crate) series: &'a Series,
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
}

#[derive(Default)]
struct Changes {
    // Set of changes to apply to database.
    set: fixed_map::Set<Change>,
    // Series removed.
    remove: HashSet<Uuid>,
    // Series added.
    add: HashSet<Uuid>,
}

impl Changes {
    #[inline]
    fn has_changes(&self) -> bool {
        !self.set.is_empty() || !self.remove.is_empty() || !self.add.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) struct Queued {
    // Series to download.
    pub(crate) series_id: Uuid,
    // Remote to download.
    pub(crate) remote_id: RemoteSeriesId,
    // Scheduled timestamp.
    pub(crate) scheduled: DateTime<Utc>,
}

/// Queue of scheduled actions.
#[derive(Default)]
struct Queue {
    // Pending series to download.
    pending: HashSet<(Uuid, RemoteSeriesId)>,
    // An item in the download queue.
    data: Vec<Queued>,
}
impl Queue {
    /// Remove the given series from the queue.
    fn remove_series(&mut self, series_id: Uuid) -> bool {
        let old = self.pending.len() + self.data.len();
        self.pending.retain(|p| p.0 != series_id);
        self.data.retain(|q| q.series_id != series_id);
        old != self.pending.len() + self.data.len()
    }
}

#[derive(Default)]
struct SeriesDatabase {
    data: Vec<Series>,
    by_id: HashMap<Uuid, usize>,
}

impl SeriesDatabase {
    /// Get a series immutably.
    fn get(&self, id: &Uuid) -> Option<&Series> {
        let &index = self.by_id.get(id)?;
        self.data.get(index)
    }

    /// Get a series mutably.
    fn get_mut(&mut self, id: &Uuid) -> Option<&mut Series> {
        let &index = self.by_id.get(id)?;
        self.data.get_mut(index)
    }

    /// Remove the series by the given identifier.
    fn remove(&mut self, id: &Uuid) -> Option<Series> {
        let index = self.by_id.remove(id)?;
        Some(self.data.remove(index))
    }
}

#[derive(Default)]
struct Database {
    /// Application configuration.
    config: Config,
    /// Remote series.
    remote_series: BTreeMap<RemoteSeriesId, Uuid>,
    /// Remote episodes.
    remote_episodes: BTreeMap<RemoteEpisodeId, Uuid>,
    /// Episode IDs to remotes.
    episodes_remotes: HashMap<Uuid, BTreeSet<RemoteEpisodeId>>,
    /// Series database.
    series: SeriesDatabase,
    /// Episodes collection.
    episodes: HashMap<Uuid, Vec<Episode>>,
    /// Seasons collection.
    seasons: HashMap<Uuid, Vec<Season>>,
    /// Episode to watch history.
    watched: BTreeMap<Uuid, Vec<Watched>>,
    /// Ordered list of things to watch.
    pending: Vec<Pending>,
    /// Keeping track of changes to be saved.
    changes: Changes,
    /// Download queue.
    queue: Queue,
}

struct Paths {
    /// Mutex to avoid clobbering the filesystem with multiple concurrent writes - but only from the same application.
    lock: tokio::sync::Mutex<()>,
    /// Path to configuration file.
    config: Box<Path>,
    /// Path where remote mappings are stored.
    remotes: Box<Path>,
    /// Path where download queue is stored.
    queue: Box<Path>,
    /// Images configuration directory.
    images: Box<Path>,
    /// Path where series are stored.
    series: Box<Path>,
    /// Watch history.
    watched: Box<Path>,
    /// Pending history.
    pending: Box<Path>,
    /// Path where episodes are stored.
    episodes: Box<Path>,
    /// Path where seasons are stored.
    seasons: Box<Path>,
}

/// Background service taking care of all state handling.
pub struct Service {
    paths: Arc<Paths>,
    db: Database,
    pub(crate) tvdb: thetvdb::Client,
    pub(crate) tmdb: themoviedb::Client,
    do_not_save: bool,
    /// Set of series which are in the process of being downloaded.
    downloading: HashSet<RemoteSeriesId>,
    /// Series IDs in the process of being downloaded.
    downloading_ids: HashSet<Uuid>,
}

impl Service {
    /// Construct and setup in-memory state of
    pub(crate) fn new() -> Result<Self> {
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

        let this = Self {
            paths: Arc::new(paths),
            db,
            tvdb,
            tmdb,
            do_not_save: false,
            downloading: HashSet::new(),
            downloading_ids: HashSet::new(),
        };

        Ok(this)
    }

    /// Get a single series.
    pub(crate) fn series(&self, id: Uuid) -> Option<&Series> {
        self.db.series.get(&id)
    }

    /// Get list of series.
    pub(crate) fn all_series(&self) -> &[Series] {
        &self.db.series.data
    }

    /// Iterator over available episodes.
    #[inline]
    pub(crate) fn episodes(&self, id: Uuid) -> &[Episode] {
        let Some(values) = self.db.episodes.get(&id) else {
            return &[];
        };

        values
    }

    /// Iterator over available seasons.
    pub(crate) fn seasons(&self, id: Uuid) -> impl Iterator<Item = &Season> {
        self.db.seasons.get(&id).into_iter().flatten()
    }

    /// Get all the watches for the given episode.
    pub(crate) fn watched(&self, episode_id: Uuid) -> &[Watched] {
        self.db
            .watched
            .get(&episode_id)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    /// Get download queue.
    pub(crate) fn queue(&self) -> &[Queued] {
        &self.db.queue.data
    }

    /// Indicates that a series is in the process of downloading.
    pub(crate) fn is_downloading(&self, remote_id: &RemoteSeriesId) -> bool {
        self.downloading.contains(remote_id)
    }

    /// Indicates that a series is in the process of downloading.
    pub(crate) fn is_downloading_id(&self, series_id: &Uuid) -> bool {
        self.downloading_ids.contains(series_id)
    }

    /// Test if episode is watched.
    pub(crate) fn watch_count(&self, episode_id: Uuid) -> usize {
        self.db
            .watched
            .get(&episode_id)
            .map(Vec::len)
            .unwrap_or_default()
    }

    /// Get season summary statistics.
    pub(crate) fn season_watched(&self, series_id: Uuid, season: SeasonNumber) -> (usize, usize) {
        let mut total = 0;
        let mut watched = 0;

        for episode in self
            .episodes(series_id)
            .iter()
            .filter(|e| e.season == season)
        {
            total += 1;
            watched += usize::from(self.watch_count(episode.id) != 0);
        }

        (watched, total)
    }

    /// Get the pending episode for the given series.
    pub(crate) fn get_pending(&self, series_id: Uuid) -> Option<&Pending> {
        self.db
            .pending
            .iter()
            .filter(|p| p.series == series_id)
            .next()
    }

    /// Return list of pending episodes.
    pub(crate) fn pending(&self) -> impl DoubleEndedIterator<Item = PendingRef<'_>> {
        self.db.pending.iter().flat_map(|p| {
            let series = self.db.series.get(&p.series)?;
            let episodes = self.db.episodes.get(&p.series)?;
            let episode = episodes.iter().find(|e| e.id == p.episode)?;
            Some(PendingRef { series, episode })
        })
    }

    /// Test if we have changes.
    pub(crate) fn has_changes(&self) -> bool {
        self.db.changes.has_changes()
    }

    /// Find updates that need to be performed.
    pub(crate) fn find_updates(
        &mut self,
        mut now: DateTime<Utc>,
    ) -> impl Future<Output = Result<Vec<Queued>>> {
        // Cache series updates for 6 hours.
        const CACHE_TIME: i64 = 3600 * 6;

        let mut interests = Vec::new();

        for s in &mut self.db.series.data {
            // Ignore series which are no longer tracked.
            if !s.tracked {
                continue;
            }

            // Note: to avoid an update loop. If this is missing the user can
            // manually try to refresh the series.
            let Some(last_modified) = s.last_modified else {
                continue;
            };

            for remote_id in &s.remote_ids {
                // Reduce the number of API requests by ensuring we don't check
                // for updates more than each CACHE_TIME interval.
                if let Some(last_sync) = s.last_sync.get(&remote_id) {
                    if now.signed_duration_since(*last_sync).num_seconds() < CACHE_TIME {
                        continue;
                    }
                }

                if s.tracked && !self.db.queue.pending.contains(&(s.id, *remote_id)) {
                    interests.push((s.id, last_modified, *remote_id));
                    s.last_sync.insert(*remote_id, now);
                    self.db.changes.set.insert(Change::Series);
                }
            }
        }

        let tvdb = self.tvdb.clone();

        async move {
            let mut queue = Vec::new();

            for (series_id, last_modified, remote_id) in interests {
                log::trace!("{series_id}/{remote_id}: checking for updates (last_modified: {last_modified})");

                match remote_id {
                    RemoteSeriesId::Tvdb { id } => {
                        let Some(update) = tvdb.series_last_modified(id).await? else {
                            continue;
                        };

                        log::trace!("{series_id}/{remote_id:?}: last modified: {update}");

                        if last_modified >= update {
                            continue;
                        }
                    }
                    // Nothing to do with the TMDB remote.
                    RemoteSeriesId::Tmdb { .. } => {
                        continue;
                    }
                    // Nothing to do with the IMDB remote.
                    RemoteSeriesId::Imdb { .. } => {
                        continue;
                    }
                }

                queue.push(Queued {
                    series_id,
                    remote_id,
                    scheduled: now,
                });

                now += Duration::minutes(1);
            }

            Ok(queue)
        }
    }

    /// Add updates to download to the queue.
    pub(crate) fn add_to_queue(&mut self, update: Vec<Queued>) {
        if update.is_empty() {
            return;
        }

        self.db.changes.set.insert(Change::Queue);

        for d in update {
            let added = self.db.queue.pending.insert((d.series_id, d.remote_id));

            if added {
                self.db.queue.data.push(d);
            }
        }

        self.db
            .queue
            .data
            .sort_by(|a, b| b.scheduled.cmp(&a.scheduled));
    }

    /// Mark an episode as watched at the given timestamp.
    pub(crate) fn watch_remaining_season(
        &mut self,
        series: Uuid,
        season: SeasonNumber,
        timestamp: DateTime<Utc>,
    ) {
        let mut last = None;

        for episode in self
            .db
            .episodes
            .get(&series)
            .into_iter()
            .flatten()
            .filter(|e| e.season == season)
        {
            if self.watch_count(episode.id) > 0 {
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
                    series,
                    episode: episode.id,
                    timestamp,
                });

            self.db.changes.set.insert(Change::Watched);
            last = Some(episode.id);
        }

        let Some(last) = last else {
            return;
        };

        self.setup_pending(series, Some(last));
    }

    /// Mark an episode as watched at the given timestamp.
    pub(crate) fn watch(&mut self, series_id: Uuid, episode_id: Uuid, timestamp: DateTime<Utc>) {
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
        self.setup_pending(series_id, Some(episode_id))
    }

    /// Skip an episode.
    pub(crate) fn skip(&mut self, series_id: Uuid, episode_id: Uuid, now: DateTime<Utc>) {
        let Some(episodes) = self.db.episodes.get(&series_id) else {
            return;
        };

        let mut it = episodes.iter();

        while let Some(episode) = it.next() {
            if episode.id == episode_id {
                break;
            }
        }

        self.db.changes.set.insert(Change::Pending);

        let Some(episode) = it.find(|e| e.has_aired(&now)) else {
            self.db.pending.retain(|p| p.series != series_id);
            return;
        };

        for pending in self
            .db
            .pending
            .iter_mut()
            .filter(|p| p.episode == episode_id)
        {
            pending.episode = episode.id;
            pending.timestamp = now;
        }
    }

    /// Select the next pending episode to use for a show.
    pub(crate) fn select_pending(&mut self, series_id: Uuid, episode_id: Uuid, now: DateTime<Utc>) {
        self.db.changes.set.insert(Change::Pending);

        // Try to modify in-place.
        if let Some(pending) = self
            .db
            .pending
            .iter_mut()
            .filter(|p| p.series == series_id)
            .next()
        {
            pending.episode = episode_id;
            pending.timestamp = now;
        } else {
            self.db.pending.push(Pending {
                series: series_id,
                episode: episode_id,
                timestamp: now,
            });
        }

        self.db
            .pending
            .sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
    }

    /// Remove all watches of the given episode.
    pub(crate) fn remove_last_episode_watch(&mut self, series_id: Uuid, episode_id: Uuid) {
        let watched = self.db.watched.entry(episode_id).or_default();
        watched.pop();
        self.db.changes.set.insert(Change::Watched);

        if watched.is_empty() {
            self.db.pending.retain(|p| p.series != series_id);

            let last_timestamp = self
                .episodes(series_id)
                .iter()
                .take_while(|e| e.id != episode_id)
                .flat_map(|e| self.watched(e.id))
                .map(|w| w.timestamp)
                .max();

            self.db.pending.push(Pending {
                series: series_id,
                episode: episode_id,
                timestamp: last_timestamp.unwrap_or_else(Utc::now),
            });

            self.db.changes.set.insert(Change::Pending);
        }
    }

    /// Remove all watches of the given episode.
    pub(crate) fn remove_season_watches(
        &mut self,
        series_id: Uuid,
        season: SeasonNumber,
        now: DateTime<Utc>,
    ) {
        let Some(episodes) = self.db.episodes.get(&series_id) else {
            return;
        };

        let mut last_timestamp = None;

        for e in episodes {
            if e.season == season {
                let _ = self.db.watched.remove(&e.id);
            } else if e.season < season {
                last_timestamp = last_timestamp
                    .into_iter()
                    .chain(self.watched(e.id).iter().map(|w| w.timestamp))
                    .max();
            }
        }

        self.db.changes.set.insert(Change::Watched);
        self.db.pending.retain(|p| p.series != series_id);
        self.db.changes.set.insert(Change::Pending);

        let Some(episodes) = self.db.episodes.get(&series_id) else {
            return;
        };

        // Find the first episode matching the given season and make that the
        // pending episode.
        if let Some(episode) = episodes
            .iter()
            .find(|e| e.season == season && e.has_aired(&now))
        {
            self.db.pending.push(Pending {
                series: series_id,
                episode: episode.id,
                timestamp: last_timestamp.unwrap_or(now),
            });
        }
    }

    /// Set up next pending episode.
    fn setup_pending(&mut self, series: Uuid, episode: Option<Uuid>) {
        let now = Utc::now();
        self.populate_pending(&now, series, episode);
    }

    /// Save changes made.
    pub(crate) fn save_changes(&mut self) -> impl Future<Output = Result<()>> {
        let changes = std::mem::take(&mut self.db.changes);

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
            .then(|| self.db.queue.data.clone());

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
            let mut remotes =
                Vec::with_capacity(&self.db.remote_series.len() + self.db.remote_episodes.len());

            for (&remote, &uuid) in &self.db.remote_series {
                remotes.push(RemoteId::Series { uuid, remote });
            }

            for (&remote, &uuid) in &self.db.remote_episodes {
                remotes.push(RemoteId::Episode { uuid, remote });
            }

            Some(remotes)
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
                let episodes = paths.episodes.join(format!("{}.json", series_id));
                let seasons = paths.seasons.join(format!("{}.json", series_id));
                let a = remove_file("episodes", &episodes);
                let b = remove_file("episodes", &seasons);
                let _ = tokio::try_join!(a, b)?;
            }

            for (series_id, episodes, seasons) in add_series {
                let episodes_path = paths.episodes.join(format!("{}.json", series_id));
                let seasons_path = paths.seasons.join(format!("{}.json", series_id));
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
        series_id: Uuid,
        last: Option<Uuid>,
    ) {
        // Remove any pending episodes for the given series.
        self.db.pending.retain(|p| p.series != series_id);
        self.db.changes.set.insert(Change::Pending);

        // Populate the next pending episode.
        let Some(episodes) = self.db.episodes.get(&series_id) else {
            return;
        };

        let mut last_timestamp = None;

        let episode = if let Some(last) = last {
            let mut it = episodes.iter();

            // Find the first episode which is after the last episode indicated.
            loop {
                let Some(e) = it.next() else {
                    break None;
                };

                if e.id == last {
                    last_timestamp = self.watched(e.id).into_iter().map(|w| w.timestamp).max();
                    break it.next();
                }
            }
        } else {
            let mut last = None;

            // Find the first episode which is *not* in our watch history.
            for episode in episodes {
                if self.watch_count(episode.id) == 0 {
                    last = Some(last.unwrap_or(episode));
                    continue;
                }

                last_timestamp = self
                    .watched(episode.id)
                    .into_iter()
                    .map(|w| w.timestamp)
                    .max();
                last = None;
            }

            last
        };

        self.db.changes.set.insert(Change::Pending);

        // Mark the first episode (that has aired).
        if let Some(e) = episode.filter(|e| e.has_aired(now)) {
            // Mark the next episode in the show as pending.
            self.db.pending.push(Pending {
                series: series_id,
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

    /// Set the theme configuration option.
    pub(crate) fn set_theme(&mut self, theme: ThemeType) {
        self.db.config.theme = theme;
        self.db.changes.set.insert(Change::Config);
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

    /// Refresh series data.
    pub(crate) fn refresh_series(
        &mut self,
        series_id: Uuid,
    ) -> Option<impl Future<Output = (Option<Uuid>, RemoteSeriesId, Result<NewSeries>)>> {
        let series = self.db.series.get(&series_id)?;
        let remote_id = series.remote_id?;
        self.downloading_ids.insert(series_id);

        let op = self.download_series(remote_id, false);

        Some(async move {
            let (_, remote_id, result) = op.await;
            (Some(series_id), remote_id, result)
        })
    }

    /// Remove the given series by ID.
    pub(crate) fn remove_series(&mut self, series_id: Uuid) {
        let _ = self.db.series.remove(&series_id);
        let _ = self.db.episodes.remove(&series_id);
        let _ = self.db.seasons.remove(&series_id);
        self.db.changes.set.insert(Change::Series);
        self.db.changes.set.insert(Change::Queue);
        self.db.changes.add.remove(&series_id);
        self.db.changes.remove.insert(series_id);

        if self.db.queue.remove_series(series_id) {
            self.db.changes.set.insert(Change::Queue);
        }
    }

    /// Enable tracking of the series with the given id.
    pub(crate) fn download_series_by_remote(
        &mut self,
        remote_id: RemoteSeriesId,
    ) -> impl Future<Output = (Option<Uuid>, RemoteSeriesId, Result<NewSeries>)> {
        self.download_series(remote_id, true)
    }

    /// Download series using a remote identifier.
    fn download_series(
        &mut self,
        remote_id: RemoteSeriesId,
        refresh_pending: bool,
    ) -> impl Future<Output = (Option<Uuid>, RemoteSeriesId, Result<NewSeries>)> {
        self.downloading.insert(remote_id);

        let tvdb = self.tvdb.clone();
        let tmdb = self.tmdb.clone();
        let series = self.db.remote_series.clone();
        let episodes = self.db.remote_episodes.clone();
        let episodes_remotes = self.db.episodes_remotes.clone();

        let lookup_series =
            move |q| Some(*series.iter().find(|(remote_id, _)| **remote_id == q)?.1);

        let id = lookup_series(remote_id);

        if let Some(id) = id {
            self.downloading_ids.insert(id);
        }

        let op = async move {
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
                    let (series, seasons) = tmdb.series(id, lookup_series).await?;

                    let mut episodes = Vec::new();

                    for season in &seasons {
                        let new_episodes = tmdb
                            .episodes(
                                id,
                                season.number,
                                |q| lookup_episode(q),
                                |id| episodes_remotes.get(&id),
                            )
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

            Ok::<_, Error>(data)
        };

        async move { (id, remote_id, op.await) }
    }

    /// If the series is already loaded in the local database, simply mark it as tracked.
    pub(crate) fn set_tracked_by_remote(&mut self, id: RemoteSeriesId) -> bool {
        let Some(&id) = self.db.remote_series.get(&id) else {
            return false;
        };

        self.track(id)
    }

    /// Set the given show as tracked.
    pub(crate) fn track(&mut self, series_id: Uuid) -> bool {
        let Some(series) = self.db.series.get_mut(&series_id) else {
            return false;
        };

        series.tracked = true;
        self.db.changes.set.insert(Change::Series);
        self.setup_pending(series_id, None);
        true
    }

    /// Disable tracking of the series with the given id.
    pub(crate) fn untrack(&mut self, series_id: Uuid) {
        if let Some(s) = self.db.series.get_mut(&series_id) {
            s.tracked = false;
            self.db.changes.set.insert(Change::Series);

            if self.db.queue.remove_series(series_id) {
                self.db.changes.set.insert(Change::Queue);
            }
        }

        self.db.pending.retain(|p| p.series != series_id);
        self.db.changes.set.insert(Change::Pending);
    }

    /// Download completed, whether it was successful or not.
    pub(crate) fn download_complete(&mut self, series_id: Option<Uuid>, remote_id: RemoteSeriesId) {
        self.downloading.remove(&remote_id);

        if let Some(series_id) = series_id {
            self.downloading_ids.remove(&series_id);
        }
    }

    /// Insert a new tracked song.
    pub(crate) fn insert_new_series(&mut self, data: NewSeries) {
        let series_id = data.series.id;

        for remote_id in &data.series.remote_ids {
            self.db.remote_series.insert(*remote_id, series_id);
        }

        for episode in &data.episodes {
            for &remote_id in &episode.remote_ids {
                self.db.remote_episodes.insert(remote_id, episode.id);
                self.db
                    .episodes_remotes
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
            self.db.series.data.push(data.series);
            self.db.series.data.sort_by(|a, b| a.title.cmp(&b.title));
            self.db.series.by_id.clear();

            for (index, s) in self.db.series.data.iter().enumerate() {
                self.db.series.by_id.insert(s.id, index);
            }
        }

        if data.refresh_pending {
            // Remove any pending episodes for the given series.
            let now = Utc::now();
            self.populate_pending(&now, series_id, None);
        }

        self.db.changes.set.insert(Change::Series);
        self.db.changes.set.insert(Change::Remotes);
        self.db.changes.remove.remove(&series_id);
        self.db.changes.add.insert(series_id);
    }

    /// Ensure that a collection of the given image ids are loaded.
    pub(crate) fn load_image(
        &self,
        id: Image,
    ) -> impl Future<Output = Result<Vec<(Image, Handle)>>> {
        let paths = self.paths.clone();
        let tvdb = self.tvdb.clone();
        let tmdb = self.tmdb.clone();

        async move {
            Ok(match id {
                Image::Tvdb(id) => cache::images(&paths.images, &tvdb, [id]).await?,
                Image::Tmdb(id) => cache::images(&paths.images, &tmdb, [id]).await?,
            })
        }
    }

    /// Prevents the service from saving anything to the filesystem.
    pub(crate) fn do_not_save(&mut self) {
        self.do_not_save = true;
    }

    /// Get existing id by remote if it exists.
    pub(crate) fn existing_by_remote_ids<I>(&self, ids: I) -> Option<Uuid>
    where
        I: IntoIterator<Item = RemoteSeriesId>,
    {
        for remote_id in ids {
            if let Some(&id) = self.db.remote_series.get(&remote_id) {
                return Some(id);
            }
        }

        None
    }

    /// Insert a new watch.
    pub(crate) fn insert_new_watch(
        &mut self,
        series_id: Uuid,
        episode_id: Uuid,
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
    pub(crate) fn clear_watches(&mut self, series_id: Uuid) {
        for (_, values) in &mut self.db.watched {
            values.retain(|w| w.series != series_id);
        }

        self.db.changes.set.insert(Change::Watched);
    }

    /// Find an episode using the given predicate.
    pub(crate) fn find_episode_by<P>(&self, series_id: Uuid, mut predicate: P) -> Option<&Episode>
    where
        P: FnMut(&Episode) -> bool,
    {
        self.episodes(series_id).iter().find(move |&e| predicate(e))
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

    db.config = match load_config(&paths.config)? {
        Some(settings) => settings,
        None => Default::default(),
    };

    if let Some(remotes) = load_array::<RemoteId>(&paths.remotes)? {
        for remote_id in remotes {
            match remote_id {
                RemoteId::Series { uuid, remote } => {
                    db.remote_series.insert(remote, uuid);
                }
                RemoteId::Episode { uuid, remote } => {
                    db.remote_episodes.insert(remote, uuid);
                    db.episodes_remotes.entry(uuid).or_default().insert(remote);
                }
            }
        }
    }

    if let Some(queue) = load_array::<Queued>(&paths.queue)? {
        for d in &queue {
            db.queue.pending.insert((d.series_id, d.remote_id));
        }

        db.queue.data = queue;
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
    let _ = tokio::fs::remove_file(path);
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
fn load_directory<T>(path: &Path) -> Result<Option<Vec<(Uuid, Vec<T>)>>>
where
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
    let f = match std::fs::File::open(&path) {
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

        output.push(serde_json::from_str(&line)?);
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

    map.into_iter().map(|(_, value)| value).collect()
}
