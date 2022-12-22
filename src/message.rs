use std::fmt;

use anyhow::{Error, Result};
use iced_native::image::Handle;
use uuid::Uuid;

use crate::model::{Image, RemoteSeriesId, SeasonNumber};
use crate::page;
use crate::service::{NewSeries, Queued};
use crate::utils::TimedOut;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Page {
    Dashboard,
    Search,
    SeriesList,
    Series(Uuid),
    Settings,
    Season(Uuid, SeasonNumber),
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

#[derive(Default, Debug, Clone)]
pub(crate) enum Message {
    /// Platform-specific events.
    CloseRequested,
    /// Setting-specific messages.
    Settings(page::settings::M),
    /// Search-specific messages.
    Search(page::search::M),
    /// SeriesList-specific messages.
    SeriesList(page::series_list::M),
    /// Series-specific messages.
    Series(page::series::M),
    /// Season-specific messages.
    Season(page::season::M),
    /// Do nothing.
    #[default]
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
    /// Series tracked.
    SeriesDownloadToTrack(Option<Uuid>, RemoteSeriesId, NewSeries),
    /// Remote series failed to download.
    SeriesDownloadFailed(Option<Uuid>, RemoteSeriesId, ErrorMessage),
    /// Refresh series data.
    RefreshSeries(Uuid),
    /// Switch series to use the given remote.
    SwitchSeries(Uuid, RemoteSeriesId),
    /// Remove the given series from the database.
    RemoveSeries(Uuid),
    /// Start tracking the series with the given remote ID.
    AddSeriesByRemote(RemoteSeriesId),
    /// Mark the given series / episode as watched.
    Watch(Uuid, Uuid),
    /// Skip an episode.
    Skip(Uuid, Uuid),
    /// Explicitly select the next pending episode.
    SelectPending(Uuid, Uuid),
    /// Weatch the remainder of all unwatched episodes in the specified season.
    WatchRemainingSeason(Uuid, SeasonNumber),
    /// Remove all matching season watches.
    RemoveSeasonWatches(Uuid, SeasonNumber),
    /// Start tracking the series with the given ID.
    Track(Uuid),
    /// Stop tracking the given show.
    Untrack(Uuid),
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
