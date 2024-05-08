use musli::api::{Endpoint, Request};
use musli::Encode;

use crate::broadcast::DashboardUpdate;

#[derive(Endpoint)]
#[endpoint(response = DashboardUpdate)]
pub enum InitializeDashboard {}

#[derive(Request, Encode)]
#[request(endpoint = InitializeDashboard)]
pub struct InitializeDashboardRequest;
