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
use api::broadcast::{DashboardDay, DashboardEpisode, DashboardSeries, DashboardUpdate};
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

fn dashboard_update(service: &Service) -> DashboardUpdate<'_> {
    let mut scheduled_rows = Vec::new();
    let mut cols = Vec::new();
    let mut count = 0;

    let page = service.config().schedule_page();

    for (n, day) in service.schedule().iter().enumerate() {
        if n % page == 0 && n > 0 {
            scheduled_rows.push(cols.drain(..).collect());
            cols.clear();
            count = 0;
        } else {
            count += 1;
        }

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
                });
            }

            series.push(DashboardSeries {
                title: &s.title,
                episodes,
            });
        }

        cols.push(DashboardDay { series });
    }

    if count > 0 {
        scheduled_rows.push(cols);
    }

    DashboardUpdate {
        config: service.config().clone(),
        days: scheduled_rows,
    }
}
