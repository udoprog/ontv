use std::fmt;

use anyhow::{Error, Result};
use iced_native::image::Handle;

use crate::model::{Image, SeasonNumber, SeriesId};
use crate::page;
use crate::service::Queued;
use crate::utils::TimedOut;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Page {
    Dashboard,
    Search,
    SeriesList,
    Series(SeriesId),
    Settings,
    Season(SeriesId, SeasonNumber),
    Downloads,
}

/// A detailed error message.
#[derive(Debug, Clone)]
pub(crate) struct ErrorMessage {
    pub(crate) message: String,
    pub(crate) causes: Vec<String>,
}

impl From<Error> for ErrorMessage {
    fn from(error: Error) -> Self {
        let message = error.to_string();

        let mut causes = Vec::new();

        for cause in error.chain().skip(1) {
            causes.push(cause.to_string());
        }

        ErrorMessage { message, causes }
    }
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

#[derive(Debug, Clone)]
pub(crate) enum Message {
    /// Platform-specific events.
    CloseRequested,
    Settings(page::settings::M),
    Dashboard(page::dashboard::Message),
    Search(page::search::Message),
    SeriesList(page::series_list::Message),
    Series(page::series::Message),
    Season(page::season::Message),
    /// Do nothing.
    Noop,
    /// Error during operation.
    Error(ErrorMessage),
    /// Save application changes.
    Save(TimedOut),
    /// Application state was saved.
    Saved,
    /// Check for updates.
    CheckForUpdates(TimedOut),
    /// Update download queue with the given items.
    UpdateDownloadQueue(Vec<Queued>),
    /// Request to navigate to the specified page.
    Navigate(Page),
    /// Navigate history by the specified stride.
    History(isize),
    /// Images have been loaded in the background.
    ImagesLoaded(Result<Vec<(Image, Handle)>, ErrorMessage>),
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
    #[inline]
    pub(crate) fn error(error: Error) -> Self {
        Self::Error(ErrorMessage::from(error))
    }
}
