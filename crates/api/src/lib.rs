pub mod config;

use core::fmt;

use chrono::NaiveDate;
use musli_core::{Decode, Encode};
use musli_web::api;
use serde::{Deserialize, Serialize};

use crate::config::Config;

/// Season number.
#[derive(
    Default,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    Encode,
    Decode,
)]
#[serde(untagged)]
#[musli(crate = musli_core)]
#[musli(untagged)]
pub enum SeasonNumber {
    /// Season used for non-numbered episodes.
    #[default]
    Specials,
    /// A regular numbered season.
    Number(u32),
}

impl SeasonNumber {
    #[inline]
    pub fn is_special(&self) -> bool {
        matches!(self, SeasonNumber::Specials)
    }

    /// Build season title.
    pub fn short(&self) -> SeasonShort<'_> {
        SeasonShort { season: self }
    }
}

impl fmt::Display for SeasonNumber {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SeasonNumber::Specials => write!(f, "Specials"),
            SeasonNumber::Number(number) => write!(f, "Season {number}"),
        }
    }
}

/// Short season number display.
pub struct SeasonShort<'a> {
    season: &'a SeasonNumber,
}

impl fmt::Display for SeasonShort<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.season {
            SeasonNumber::Specials => "S".fmt(f),
            SeasonNumber::Number(n) => n.fmt(f),
        }
    }
}

#[derive(Encode, Decode)]
#[musli(crate = musli_core)]
pub struct DashboardEpisode<'a> {
    /// Episode name.
    #[musli(default, skip_encoding_if = Option::is_none)]
    pub name: Option<&'a str>,
    /// Absolute number in the series.
    #[musli(default, skip_encoding_if = Option::is_none)]
    pub absolute_number: Option<u32>,
    /// Season number.
    #[musli(default, skip_encoding_if = SeasonNumber::is_special)]
    pub season: SeasonNumber,
    /// Episode number inside of its season.
    pub number: u32,
}

#[derive(Encode, Decode)]
#[musli(crate = musli_core)]
pub struct DashboardSeries<'a> {
    pub title: &'a str,
    pub episodes: Vec<DashboardEpisode<'a>>,
}

#[derive(Encode, Decode)]
#[musli(crate = musli_core)]
pub struct DashboardDay<'a> {
    #[musli(with = musli::serde)]
    pub date: NaiveDate,
    pub series: Vec<DashboardSeries<'a>>,
}

#[derive(Encode, Decode)]
#[musli(crate = musli_core)]
pub struct DashboardUpdateEvent<'a> {
    pub config: Config,
    pub days: Vec<DashboardDay<'a>>,
}

#[derive(Encode, Decode)]
#[musli(crate = musli_core)]
pub struct RequestDashboard;

api::define! {
    pub type InitializeDashboard;

    impl Endpoint for InitializeDashboard {
        impl Request for RequestDashboard;
        type Response<'de> = DashboardUpdateEvent<'de>;
    }

    /// Dashboard update.
    pub type DashboardUpdate;

    impl Broadcast for DashboardUpdate {
        impl<'de> Event for DashboardUpdateEvent<'de>;
    }
}
