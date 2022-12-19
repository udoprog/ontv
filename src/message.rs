use std::fmt;

use anyhow::{Error, Result};
use iced_native::image::Handle;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::model::{Image, SeriesId};
use crate::page;
use crate::service::SeriesData;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Page {
    Dashboard,
    Search,
    SeriesList,
    Series(Uuid),
    Settings,
    Season(Uuid, Option<u32>),
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
    /// Original loading completed.
    Loaded(Vec<(Image, Handle)>),
    /// Error during operation.
    Error(ErrorMessage),
    /// Actually save configuration.
    SaveConfig,
    /// Configuration saved and whether it was successful or not.
    SavedConfig,
    /// Request to navigate to the specified page.
    Navigate(Page),
    /// Setting-specific messages.
    Settings(page::settings::M),
    /// Dashboard-specific messages.
    #[allow(unused)]
    Dashboard(page::dashboard::M),
    /// Search-specific messages.
    Search(page::search::M),
    /// Series tracked.
    SeriesTracked(SeriesData, Vec<(Image, Handle)>),
    /// Start tracking the series with the given ID.
    Track(SeriesId),
    /// Stop tracking the given show.
    Untrack(SeriesId),
}

impl From<Result<()>> for Message {
    #[inline]
    fn from(result: Result<()>) -> Self {
        match result {
            Ok(()) => Message::Noop,
            Err(error) => Message::error(error),
        }
    }
}

impl Message {
    /// Construct an error message with detailed information.
    pub(crate) fn error(error: Error) -> Self {
        let message = error.to_string();

        let mut causes = Vec::new();

        for cause in error.chain().skip(1) {
            causes.push(cause.to_string());
        }

        Self::Error(ErrorMessage { message, causes })
    }
}
