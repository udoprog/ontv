pub mod cache;
pub mod config;
pub mod model;

use jiff::civil::Date;
use musli_core::{Decode, Encode};
use musli_web::api;

pub use self::model::{ImageExt, ImageHash, ImageHint, ImageSizeHint, ImageV2, SeasonNumber};
use crate::config::Config;

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
    pub poster: Option<ImageV2>,
}

#[derive(Encode, Decode)]
#[musli(crate = musli_core)]
pub struct DashboardDay<'a> {
    #[musli(with = musli::serde)]
    pub date: Date,
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
