use musli::Encode;
use musli_axum::api::{Marker, Request};

use crate::broadcast::DashboardUpdate;

pub enum InitializeDashboard {}

impl Marker for InitializeDashboard {
    type Type<'de> = DashboardUpdate<'de>;
}

#[derive(Encode)]
pub struct InitializeDashboardRequest;

impl Request for InitializeDashboardRequest {
    const KIND: &'static str = "initialize-dashboard";
    type Marker = InitializeDashboard;
}
