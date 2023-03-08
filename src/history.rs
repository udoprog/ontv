use iced::widget::scrollable::RelativeOffset;

use crate::assets::Assets;
use crate::model::{MovieId, SeasonNumber, SeriesId};
use crate::page;

/// The current page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Page {
    Dashboard,
    WatchNext(page::watch_next::PageState),
    Search,
    SeriesList,
    Series(SeriesId),
    Movie(MovieId),
    Settings,
    Season(SeriesId, SeasonNumber),
    Queue,
    Errors,
}

pub(crate) struct History {
    // History entries.
    history: Vec<(Page, RelativeOffset)>,
    // Current history entry.
    history_index: usize,
    // History has changed.
    history_changed: bool,
}

impl History {
    /// Construct a new empty history.
    pub(crate) fn new() -> Self {
        Self {
            history: vec![(Page::Dashboard, RelativeOffset::default())],
            history_index: 0,
            history_changed: false,
        }
    }

    /// Get the current page.
    pub(crate) fn page(&self) -> Option<&Page> {
        Some(&self.history.get(self.history_index)?.0)
    }

    /// Get history mutably.
    pub(crate) fn page_mut(&mut self) -> Option<&mut Page> {
        Some(&mut self.history.get_mut(self.history_index)?.0)
    }

    /// Push a history entry.
    pub(crate) fn push_history(&mut self, assets: &mut Assets, page: Page) {
        assets.clear();

        while self.history_index + 1 < self.history.len() {
            self.history.pop();
        }

        self.history.push((page, RelativeOffset::default()));
        self.history_index += 1;
        self.history_changed = true;
    }

    /// Update scroll location in history.
    pub(crate) fn history_scroll(&mut self, scroll: RelativeOffset) {
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
    pub(crate) fn history_change(&mut self) -> Option<&(Page, RelativeOffset)> {
        if !self.history_changed {
            return None;
        }

        self.history_changed = false;
        self.history.get(self.history_index)
    }
}
