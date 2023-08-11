use std::collections::{HashSet, VecDeque};

use chrono::{Duration, NaiveDate, Utc};

use crate::error::{ErrorId, ErrorInfo};

pub(crate) struct State {
    /// Current error identifiers.
    error_ids: HashSet<ErrorId>,
    /// Errors accumulated.
    errors: VecDeque<ErrorInfo>,
    /// Indicates that the whole application is busy saving.
    saving: bool,
    /// Naive today date.
    today: NaiveDate,
}

impl State {
    /// Construct a new empty application state.
    #[inline]
    pub fn new(today: NaiveDate) -> Self {
        Self {
            error_ids: HashSet::new(),
            errors: VecDeque::new(),
            saving: false,
            today,
        }
    }

    /// Access today's date.
    pub(crate) fn today(&self) -> &NaiveDate {
        &self.today
    }

    /// Set today's date.
    pub(crate) fn set_today(&mut self, today: NaiveDate) {
        self.today = today;
    }

    /// Handle an error.
    pub(crate) fn handle_error(&mut self, error: ErrorInfo) {
        tracing::error!(?error, "Error");

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
}
