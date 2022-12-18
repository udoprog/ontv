use std::sync::Arc;

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

#[derive(Default, Debug, Clone)]
pub(crate) enum Message {
    /// Do nothing.
    #[default]
    Noop,
    /// Error during operation.
    Error(String),
    /// Setup procedure finished running.
    Setup((page::settings::State, Option<Arc<Error>>)),
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
    ImagesLoaded,
    /// Start tracking the series with the given ID.
    Track(TheTvDbSeriesId),
    /// Stop tracking the given show.
    Untrack(TheTvDbSeriesId),
}
