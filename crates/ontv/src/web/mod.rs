#[cfg(feature = "bundle")]
#[path = "bundle.rs"]
mod r#impl;

#[cfg(not(feature = "bundle"))]
#[path = "nonbundle.rs"]
mod r#impl;

pub(crate) use self::r#impl::BIND;

mod api;
mod ws;

use crate::model::Config;
use crate::service::Service;

/// Error type for web module.
pub struct WebError {
    kind: WebErrorKind,
}

impl WebError {
    fn not_found() -> Self {
        Self {
            kind: WebErrorKind::NotFound,
        }
    }
}

impl IntoResponse for WebError {
    fn into_response(self) -> axum::response::Response {
        match self.kind {
            WebErrorKind::Error(err) => {
                let body = format!("Internal server error: {err}");
                (StatusCode::INTERNAL_SERVER_ERROR, body).into_response()
            }
            WebErrorKind::NotFound => (StatusCode::NOT_FOUND, "Not Found").into_response(),
        }
    }
}

enum WebErrorKind {
    Error(anyhow::Error),
    NotFound,
}

impl From<anyhow::Error> for WebError {
    #[inline]
    fn from(err: anyhow::Error) -> Self {
        Self {
            kind: WebErrorKind::Error(err),
        }
    }
}

use std::future::Future;
use std::net::SocketAddr;
use std::sync::Arc;

use ::api::{DashboardDay, DashboardEpisode, DashboardSeries, DashboardUpdateEvent};
use anyhow::Result;
use axum::http::StatusCode;
use axum::response::IntoResponse;
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
    let router = router.route("/ws", get(ws::entry));
    let router = router.route("/api/image/{hint}/{image}", get(api::image));
    router
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
                poster: s.poster().cloned(),
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
