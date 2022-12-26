use std::collections::{HashSet, VecDeque};
use std::future::Future;

use anyhow::Result;
use chrono::{Duration, Utc};

use crate::assets::Assets;
use crate::error::{ErrorId, ErrorInfo};
use crate::model::{SeasonNumber, SeriesId};
use crate::service::{NewSeries, Service};

/// The current page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Page {
    Dashboard,
    Search,
    SeriesList,
    Series(SeriesId),
    Settings,
    Season(SeriesId, SeasonNumber),
    Queue,
    Errors,
}

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
    /// Current error identifiers.
    error_ids: HashSet<ErrorId>,
    /// Errors accumulated.
    errors: VecDeque<ErrorInfo>,
    /// Indicates that the whole application is busy saving.
    saving: bool,
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
            error_ids: HashSet::new(),
            errors: VecDeque::new(),
            saving: false,
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
    pub(crate) fn handle_error(&mut self, error: ErrorInfo) {
        log::error!("error: {error}");

        self.saving = false;
        self.error_ids.extend(error.id);
        self.errors.push_front(error);

        let expires_at = Utc::now() - Duration::minutes(10);

        while let Some(e) = self.errors.back() {
            if e.timestamp > expires_at {
                break;
            }

            self.errors.pop_back();
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

    /// Refresh series data.
    pub(crate) fn refresh_series(
        &mut self,
        series_id: &SeriesId,
    ) -> Option<impl Future<Output = Result<Option<NewSeries>>>> {
        let s = self.service.series(series_id)?;
        let remote_id = s.remote_id?;
        let none_if_match = s.last_etag.clone();
        Some(
            self.service
                .download_series(&remote_id, false, none_if_match.as_ref()),
        )
    }

    #[inline]
    pub(crate) fn errors(&self) -> impl ExactSizeIterator<Item = &ErrorInfo> + DoubleEndedIterator {
        self.errors.iter()
    }

    #[inline]
    pub(crate) fn get_error(&self, id: ErrorId) -> Option<&ErrorInfo> {
        self.errors
            .iter()
            .find(|e| matches!(&e.id, Some(error_id) if *error_id == id))
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

    #[inline]
    pub(crate) fn missing_poster(&self) -> iced_native::image::Handle {
        self.assets.missing_poster(self.service.theme())
    }
}
