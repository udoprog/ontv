use core::fmt;

use musli::{Decode, Encode};
use serde::{Deserialize, Serialize};

#[derive(Default, Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Encode, Decode)]
#[serde(rename_all = "kebab-case")]
pub enum ThemeType {
    Light,
    #[default]
    Dark,
}

/// Helper type to wrap a sensitive string value so that it's hard to accidentally log.
#[derive(Default, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Encode, Decode)]
#[serde(transparent)]
#[musli(transparent)]
pub struct Secret {
    string: String,
}

impl Secret {
    /// Construct a new secret string.
    pub fn new(string: String) -> Self {
        Self { string }
    }

    /// Set the value of the secret.
    pub fn set(&mut self, string: String) {
        self.string = string;
    }

    /// Get the underlying string.
    pub fn as_str(&self) -> &str {
        &self.string
    }
}

impl fmt::Display for Secret {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("*")
    }
}

impl fmt::Debug for Secret {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Secret").field(&"*").finish()
    }
}

/// The state for the settings page.
#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct Config {
    #[serde(default)]
    #[musli(default)]
    pub theme: ThemeType,
    #[serde(default)]
    #[musli(default)]
    pub tvdb_legacy_apikey: Secret,
    #[serde(default)]
    #[musli(default)]
    pub tmdb_api_key: Secret,
    #[serde(default = "default_days")]
    #[musli(default = default_days)]
    pub schedule_duration_days: u64,
    #[serde(default = "default_dashboard_limit")]
    #[musli(default = default_dashboard_limit)]
    pub dashboard_limit: usize,
    #[serde(default = "default_dashboard_page")]
    #[musli(default = default_dashboard_page)]
    pub dashboard_page: usize,
    #[serde(default = "default_schedule_limit")]
    #[musli(default = default_schedule_limit)]
    pub schedule_limit: usize,
    #[serde(default = "default_schedule_page")]
    #[musli(default = default_schedule_page)]
    pub schedule_page: usize,
}

impl Config {
    pub fn dashboard_limit(&self) -> usize {
        self.dashboard_limit.max(1) * self.dashboard_page.max(1)
    }

    pub fn dashboard_page(&self) -> usize {
        self.dashboard_page.max(1)
    }

    pub fn schedule_page(&self) -> usize {
        self.schedule_page.max(1)
    }
}

impl Default for Config {
    #[inline]
    fn default() -> Self {
        Self {
            theme: Default::default(),
            tvdb_legacy_apikey: Default::default(),
            tmdb_api_key: Default::default(),
            schedule_duration_days: default_days(),
            dashboard_limit: default_dashboard_limit(),
            dashboard_page: default_dashboard_page(),
            schedule_limit: default_schedule_limit(),
            schedule_page: default_schedule_page(),
        }
    }
}

#[inline]
fn default_days() -> u64 {
    7
}

#[inline]
fn default_dashboard_limit() -> usize {
    1
}

#[inline]
fn default_dashboard_page() -> usize {
    6
}

#[inline]
fn default_schedule_limit() -> usize {
    1
}

#[inline]
fn default_schedule_page() -> usize {
    7
}
