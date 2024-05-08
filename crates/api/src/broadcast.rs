use musli::{Decode, Encode};
use musli_axum::api::{Broadcast, Marker};

use crate::config::Config;

pub enum DashboardUpdateBroadcast {}

impl Marker for DashboardUpdateBroadcast {
    type Type<'de> = DashboardUpdate<'de>;
}

impl Broadcast for DashboardUpdateBroadcast {
    const KIND: &'static str = "dashboard-update";
}

#[derive(Encode, Decode)]
pub struct DashboardEpisode<'a> {
    pub name: Option<&'a str>,
}

#[derive(Encode, Decode)]
pub struct DashboardSeries<'a> {
    pub title: &'a str,
    pub episodes: Vec<DashboardEpisode<'a>>,
}

#[derive(Encode, Decode)]
pub struct DashboardDay<'a> {
    pub series: Vec<DashboardSeries<'a>>,
}

#[derive(Encode, Decode)]
pub struct DashboardUpdate<'a> {
    pub config: Config,
    pub days: Vec<Vec<DashboardDay<'a>>>,
}
