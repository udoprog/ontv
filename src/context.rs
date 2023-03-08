use std::future::Future;

use anyhow::Result;

use crate::assets::Assets;
use crate::history::{History, Page};
use crate::model::{RemoteSeriesId, SeriesId};
use crate::service::{NewSeries, Service};
use crate::state::State;

/// Context reference.
pub(crate) struct CtxtRef<'a> {
    pub(crate) state: &'a State,
    pub(crate) service: &'a Service,
    pub(crate) assets: &'a Assets,
}

impl<'a> CtxtRef<'a> {
    #[inline]
    pub(crate) fn warning_text(&self) -> iced::theme::Text {
        crate::style::warning_text(self.service.theme())
    }

    #[inline]
    pub(crate) fn missing_poster(&self) -> iced_native::image::Handle {
        self.assets.missing_poster(self.service.theme())
    }
}

/// Mutable context passed down across pages.
pub(crate) struct Ctxt<'a> {
    pub(crate) state: &'a mut State,
    /// Mutable history.
    pub(crate) history: &'a mut History,
    /// Data service.
    pub(crate) service: &'a mut Service,
    /// Asset loader.
    pub(crate) assets: &'a mut Assets,
}

impl<'a> Ctxt<'a> {
    /// Push history.
    pub(crate) fn push_history(&mut self, page: Page) {
        self.history.push_history(self.assets, page);
    }

    /// Remove a series.
    pub(crate) fn remove_series(&mut self, series_id: &SeriesId) {
        if matches!(self.history.page(), Some(Page::Series(id) | Page::Season(id, _)) if *id == *series_id)
        {
            self.history.push_history(self.assets, Page::Dashboard);
        }

        self.service.remove_series(series_id);
    }

    /// Refresh series data.
    pub(crate) fn download_series_by_id(
        &mut self,
        id: &SeriesId,
        remote_id: &RemoteSeriesId,
        force: bool,
    ) -> impl Future<Output = Result<Option<NewSeries>>> {
        let none_if_match = if force {
            None
        } else {
            self.service.last_etag(id, remote_id).cloned()
        };

        self.service
            .download_series(&remote_id, none_if_match.as_ref())
    }
}
