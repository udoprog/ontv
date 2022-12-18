use std::fmt;

use anyhow::Error;
use serde::{Deserialize, Serialize};

use crate::{model::TheTvDbSeriesId, page};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Page {
    Dashboard,
    Search,
    Settings,
}

#[derive(Default, Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ThemeType {
    #[default]
    Light,
    Dark,
}

/// A detailed error message.
#[derive(Debug, Clone)]
pub(crate) struct ErrorMessage {
    message: String,
    causes: Vec<String>,
}

impl fmt::Display for ErrorMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.causes.is_empty() {
            return self.message.fmt(f);
        }

        writeln!(f, "{}", self.message)?;

        for cause in &self.causes {
            writeln!(f, "caused by: {}", cause)?;
        }

        Ok(())
    }
}

#[derive(Default, Debug, Clone)]
pub(crate) enum Message {
    /// Do nothing.
    #[default]
    Noop,
    /// Error during operation.
    Error(ErrorMessage),
    /// Actually save configuration.
    SaveConfig,
    /// Configuration saved and whether it was successful or not.
    SavedConfig(bool),
    /// Request to navigate to the specified page.
    Navigate(Page),
    /// Setting-specific messages.
    Settings(page::settings::SettingsMessage),
    /// Dashboard-specific messages.
    #[allow(unused)]
    Dashboard(page::dashboard::DashboardMessage),
    /// Search-specific messages.
    Search(page::search::SearchMessage),
    /// Series tracked.
    SeriesTracked,
    /// Images have been loaded.
    ImageLoaded,
    /// Start tracking the series with the given ID.
    Track(TheTvDbSeriesId),
    /// Stop tracking the given show.
    Untrack(TheTvDbSeriesId),
}

impl Message {
    /// Construct an error message with detailed information.
    pub(crate) fn error(error: Error) -> Self {
        let mut message = error.to_string();

        let mut causes = Vec::new();

        for cause in error.chain().skip(1) {
            causes.push(cause.to_string());
        }

        Self::Error(ErrorMessage { message, causes })
    }
}
