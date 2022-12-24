use std::collections::{HashSet, VecDeque};
use std::future::Future;

use anyhow::Result;

use crate::assets::Assets;
use crate::message::{ErrorMessage, Page};
use crate::model::{RemoteSeriesId, SeriesId};
use crate::service::{NewSeries, Service};

#[derive(Debug, Clone)]
pub(crate) struct SeriesDownload {
    remote_id: RemoteSeriesId,
    result: Result<NewSeries, ErrorMessage>,
}

const ERRORS: usize = 5;

pub(crate) struct State {
    /// Data service.
    pub(crate) service: Service,
    /// Asset loader.
    pub(crate) assets: Assets,
    // History entries.
    history: Vec<(Page, f32)>,
    // Current history entry.
    history_index: usize,
    // History has changed.
    history_changed: bool,
    /// Errors accumulated.
    errors: VecDeque<ErrorMessage>,
    /// Indicates that the whole application is busy loading something.
    saving: bool,
    /// Set of series which are in the process of being downloaded.
    downloading: HashSet<RemoteSeriesId>,
}

impl State {
    /// Construct a new empty application state.
    #[inline]
    pub fn new(service: Service, assets: Assets) -> Self {
        Self {
            service,
            assets,
            history: vec![(Page::Dashboard, 0.0)],
            history_index: 0,
            history_changed: false,
            errors: VecDeque::new(),
            saving: false,
            downloading: HashSet::new(),
        }
    }

    /// Get the current page.
    pub(crate) fn page(&self) -> Option<&Page> {
        Some(&self.history.get(self.history_index)?.0)
    }

    /// Push a history entry.
    pub(crate) fn push_history(&mut self, page: Page) {
        self.assets.clear();

        while self.history_index + 1 < self.history.len() {
            self.history.pop();
        }

        self.history.push((page, 0.0));
        self.history_index += 1;
        self.history_changed = true;
    }

    /// Update scroll location in history.
    pub(crate) fn history_scroll(&mut self, scroll: f32) {
        if let Some((_, s)) = self.history.get_mut(self.history_index) {
            *s = scroll;
        }
    }

    /// Navigate the current history.
    pub(crate) fn history(&mut self, relative: isize) {
        if relative > 0 {
            self.history_index = self.history_index.saturating_add(relative as usize);
        } else if relative < 0 {
            self.history_index = self.history_index.saturating_sub(-relative as usize);
        }

        self.history_index = self.history_index.min(self.history.len().saturating_sub(1));
        self.history_changed = true;
    }

    /// Acquire history scroll to restore.
    pub(crate) fn history_change(&mut self) -> Option<(Page, f32)> {
        if !self.history_changed {
            return None;
        }

        self.history_changed = false;
        Some(*self.history.get(self.history_index)?)
    }

    /// Handle an error.
    pub(crate) fn handle_error(&mut self, error: ErrorMessage) {
        log::error!("error: {error}");

        self.saving = false;
        self.errors.push_back(error);

        if self.errors.len() > ERRORS {
            self.errors.pop_front();
        }
    }

    /// Remove a series.
    pub(crate) fn remove_series(&mut self, series_id: &SeriesId) {
        if matches!(self.page(), Some(Page::Series(id) | Page::Season(id, _)) if *id == *series_id)
        {
            self.push_history(Page::Dashboard);
        }

        self.service.remove_series(series_id);
    }

    /// Download completed, whether it was successful or not.
    pub(crate) fn download_complete(&mut self, remote_id: RemoteSeriesId) {
        self.downloading.remove(&remote_id);
    }

    /// Indicates that a series is in the process of downloading.
    pub(crate) fn is_downloading(&self, remote_id: &RemoteSeriesId) -> bool {
        self.downloading.contains(remote_id)
    }

    /// Refresh series data.
    pub(crate) fn refresh_series(
        &mut self,
        series_id: &SeriesId,
    ) -> Option<impl Future<Output = SeriesDownload>> {
        let remote_id = self.service.series(series_id)?.remote_id?;

        self.downloading.insert(remote_id);

        let op = self.service.download_series(&remote_id, false);

        Some(async move {
            SeriesDownload {
                remote_id,
                result: op.await.map_err(Into::into),
            }
        })
    }

    /// Download a series by remote.
    pub(crate) fn download_series_by_remote(
        &mut self,
        remote_id: &RemoteSeriesId,
    ) -> impl Future<Output = (RemoteSeriesId, Result<NewSeries>)> {
        self.downloading.insert(*remote_id);
        let op = self.service.download_series_by_remote(remote_id);
        let remote_id = *remote_id;
        async move { (remote_id, op.await) }
    }

    #[inline]
    pub(crate) fn errors(&self) -> impl Iterator<Item = &ErrorMessage> {
        self.errors.iter()
    }

    #[inline]
    pub(crate) fn is_saving(&self) -> bool {
        self.saving
    }

    #[inline]
    pub(crate) fn set_saving(&mut self, saving: bool) {
        self.saving = saving;
    }

    #[inline]
    pub(crate) fn warning_text(&self) -> iced::theme::Text {
        crate::style::warning_text(self.service.theme())
    }

    /// Handle a series download.
    pub(crate) fn handle_series_download(&mut self, download: SeriesDownload) {
        match download.result {
            Ok(data) => {
                self.service.insert_new_series(data);
            }
            Err(error) => {
                self.handle_error(error);
            }
        }

        self.download_complete(download.remote_id);
    }
}
