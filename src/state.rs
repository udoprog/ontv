use std::collections::{HashSet, VecDeque};
use std::future::Future;

use anyhow::Result;
use uuid::Uuid;

use crate::assets::Assets;
use crate::message::{ErrorMessage, Page};
use crate::model::RemoteSeriesId;
use crate::service::{NewSeries, Service};

const ERRORS: usize = 5;

pub(crate) struct State {
    /// Data service.
    pub(crate) service: Service,
    /// Asset loader.
    pub(crate) assets: Assets,
    // History entries.
    history: Vec<Page>,
    // Current history entry.
    history_index: usize,
    /// Errors accumulated.
    errors: VecDeque<ErrorMessage>,
    /// Indicates that the whole application is busy loading something.
    loading: bool,
    /// Set of series which are in the process of being downloaded.
    downloading: HashSet<RemoteSeriesId>,
    /// Series IDs in the process of being downloaded.
    downloading_ids: HashSet<Uuid>,
}

impl State {
    /// Construct a new empty application state.
    #[inline]
    pub(crate) fn new(service: Service, assets: Assets) -> Self {
        Self {
            service,
            assets,
            history: vec![Page::Dashboard],
            history_index: 0,
            errors: VecDeque::new(),
            loading: false,
            downloading: HashSet::new(),
            downloading_ids: HashSet::new(),
        }
    }

    /// Get the current page.
    pub(crate) fn page(&self) -> Option<&Page> {
        self.history.get(self.history_index)
    }

    /// Push a history entry.
    pub(crate) fn push_history(&mut self, page: Page) {
        self.assets.clear();

        while self.history_index + 1 < self.history.len() {
            self.history.pop();
        }

        self.history.push(page);
        self.history_index += 1;
    }

    /// Handle an error.
    pub(crate) fn handle_error(&mut self, error: ErrorMessage) {
        log::error!("error: {error}");

        self.loading = false;
        self.errors.push_back(error);

        if self.errors.len() > ERRORS {
            self.errors.pop_front();
        }
    }

    /// Remove a series.
    pub(crate) fn remove_series(&mut self, series_id: Uuid) {
        if matches!(self.page(), Some(&Page::Series(id) | &Page::Season(id, _)) if id == series_id)
        {
            self.push_history(Page::Dashboard);
        }

        self.service.remove_series(series_id);
    }

    /// Navigate the current history.
    pub(crate) fn history(&mut self, relative: isize) {
        if relative > 0 {
            self.history_index = self.history_index.saturating_add(relative as usize);
        } else if relative < 0 {
            self.history_index = self.history_index.saturating_sub(-relative as usize);
        }

        self.history_index = self.history_index.min(self.history.len().saturating_sub(1));
    }

    /// Download completed, whether it was successful or not.
    pub(crate) fn download_complete(&mut self, series_id: Option<Uuid>, remote_id: RemoteSeriesId) {
        self.downloading.remove(&remote_id);

        if let Some(series_id) = series_id {
            self.downloading_ids.remove(&series_id);
        }
    }

    /// Indicates that a series is in the process of downloading.
    pub(crate) fn is_downloading(&self, remote_id: &RemoteSeriesId) -> bool {
        self.downloading.contains(remote_id)
    }

    /// Indicates that a series is in the process of downloading.
    pub(crate) fn is_downloading_id(&self, series_id: &Uuid) -> bool {
        self.downloading_ids.contains(series_id)
    }

    /// Refresh series data.
    pub(crate) fn refresh_series(
        &mut self,
        series_id: Uuid,
    ) -> Option<impl Future<Output = (Option<Uuid>, RemoteSeriesId, Result<NewSeries>)>> {
        let remote_id = self.service.series(series_id)?.remote_id?;

        self.downloading.insert(remote_id);
        self.downloading_ids.insert(series_id);

        let (_, op) = self.service.download_series(remote_id, false);

        Some(async move { (Some(series_id), remote_id, op.await) })
    }

    /// Download a series by remote.
    pub(crate) fn download_series_by_remote(
        &mut self,
        remote_id: RemoteSeriesId,
    ) -> impl Future<Output = (Option<Uuid>, RemoteSeriesId, Result<NewSeries>)> {
        self.downloading.insert(remote_id);
        let (id, op) = self.service.download_series_by_remote(remote_id);

        if let Some(id) = id {
            self.downloading_ids.insert(id);
        }

        async move { (id, remote_id, op.await) }
    }

    #[inline]
    pub(crate) fn errors(&self) -> impl Iterator<Item = &ErrorMessage> {
        self.errors.iter()
    }

    #[inline]
    pub(crate) fn is_loading(&self) -> bool {
        self.loading
    }

    #[inline]
    pub(crate) fn set_loading(&mut self, loading: bool) {
        self.loading = loading;
    }
}
