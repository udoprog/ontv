#[cfg(feature = "bundle")]
#[path = "bundle.rs"]
mod r#impl;

#[cfg(not(feature = "bundle"))]
#[path = "api.rs"]
mod r#impl;

pub(crate) use self::r#impl::BIND;

mod ws;

use crate::model::Config;
use crate::service::Service;

use std::future::Future;
use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use api::{DashboardDay, DashboardEpisode, DashboardSeries, DashboardUpdateEvent};
use axum::routing::get;
use axum::{Extension, Router};
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tower_http::cors::{AllowMethods, AllowOrigin, CorsLayer};

pub(crate) fn setup(
    listener: TcpListener,
    service: Arc<RwLock<Service>>,
) -> Result<impl Future<Output = Result<()>>> {
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::any())
        .allow_methods(AllowMethods::any());

    let app = self::r#impl::router().layer(Extension(service)).layer(cors);

    let service = axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    );

    Ok(async move {
        service.await?;
        Ok(())
    })
}

fn common_routes(router: Router) -> Router {
    router.route("/ws", get(ws::entry))
}

fn dashboard_update(service: &Service) -> DashboardUpdateEvent<'_> {
    let mut days = Vec::new();

    for (n, day) in service.schedule().iter().enumerate() {
        let mut it = day
            .schedule
            .iter()
            .flat_map(|sched| {
                service
                    .series(&sched.series_id)
                    .into_iter()
                    .map(move |series| (series, sched))
            })
            .peekable();

        let mut series = Vec::new();

        while let Some((s, schedule)) = it.next() {
            let mut episodes = Vec::new();

            for episode_id in &schedule.episodes {
                let Some(e) = service.episode(episode_id) else {
                    continue;
                };

                episodes.push(DashboardEpisode {
                    name: e.as_ref().name.as_deref(),
                    absolute_number: e.as_ref().absolute_number,
                    season: e.as_ref().season,
                    number: e.as_ref().number,
                });
            }

            series.push(DashboardSeries {
                title: &s.title,
                episodes,
            });
        }

        days.push(DashboardDay {
            date: day.date,
            series,
        });
    }

    DashboardUpdateEvent {
        config: service.config().clone(),
        days,
    }
}
