use musli::api::Endpoint;
use musli::{Decode, Encode};

use crate::config::Config;
use crate::model::{Episode, ScheduledDay, Series};
use crate::{EpisodeId, SeriesId};

#[derive(Endpoint)]
#[endpoint(response = DashboardUpdate)]
pub enum DashboardUpdateBroadcast {}

#[derive(Endpoint)]
#[endpoint(response = SeriesUpdate)]
pub enum SeriesUpdateBroadcast {}

#[derive(Encode, Decode)]
pub struct DashboardEpisode<'a> {
    pub id: EpisodeId,
    pub name: Option<&'a str>,
}

#[derive(Encode, Decode)]
pub struct DashboardSeries<'a> {
    pub id: SeriesId,
    pub title: &'a str,
}

#[derive(Encode, Decode)]
pub struct DashboardUpdate {
    pub config: Config,
    pub schedule: Vec<ScheduledDay>,
    pub series: Vec<Series>,
    pub episodes: Vec<Episode>,
}

#[derive(Encode, Decode)]
pub struct SeriesUpdate {}
