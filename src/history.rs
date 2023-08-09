use iced::widget::scrollable::RelativeOffset;
use serde::{Deserialize, Serialize};

use crate::assets::Assets;
use crate::page;

/// The current page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Page {
    Dashboard,
    WatchNext(page::watch_next::State),
    Search(page::search::State),
    SeriesList,
    MoviesList,
    Series(page::series::State),
    Movie(page::movie::State),
    Settings,
    Season(page::season::State),
    Queue(page::queue::State),
    Errors,
}

#[derive(Default)]
pub(crate) struct HistoryMutations {
    relative: Option<isize>,
    push: Option<Page>,
}

impl HistoryMutations {
    /// Test if there are any recorded mutations.
    fn has_any(&self) -> bool {
        self.relative.is_some() || self.push.is_some()
    }

    /// Navigate history.
    pub(crate) fn navigate(&mut self, relative: isize) {
        self.relative = Some(relative);
    }

    /// Push a history entry.
    pub(crate) fn push_history(&mut self, assets: &mut Assets, page: Page) {
        assets.clear();
        self.push = Some(page);
    }
}

pub(crate) struct History {
    // History entries.
    history: Vec<(Page, RelativeOffset)>,
    // Current history entry.
    history_index: usize,
}

impl History {
    /// Construct a new empty history.
    pub(crate) fn new() -> Self {
        Self {
            history: vec![(Page::Dashboard, RelativeOffset::default())],
            history_index: 0,
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

    /// Update scroll location in history.
    pub(crate) fn history_scroll(&mut self, scroll: RelativeOffset) {
        if let Some((_, s)) = self.history.get_mut(self.history_index) {
            *s = scroll;
        }
    }

    /// Apply an existing history mutation.
    pub(crate) fn apply_mutation(
        &mut self,
        mutation: &mut HistoryMutations,
    ) -> Option<&(Page, RelativeOffset)> {
        if !mutation.has_any() {
            return None;
        }

        if let Some(relative) = mutation.relative.take() {
            self.history_index = match relative.signum() {
                1 => self.history_index.saturating_add(relative as usize),
                -1 => self.history_index.saturating_sub(-relative as usize),
                _ => 0,
            }
            .min(self.history.len().saturating_sub(1));
        }

        if let Some(page) = mutation.push.take() {
            while self.history_index + 1 < self.history.len() {
                self.history.pop();
            }

            self.history.push((page, RelativeOffset::default()));
            self.history_index += 1;
        }

        self.history.get(self.history_index)
    }
}
