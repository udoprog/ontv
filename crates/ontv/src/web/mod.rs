#[cfg(feature = "bundle")]
#[path = "bundle.rs"]
mod r#impl;

#[cfg(not(feature = "bundle"))]
#[path = "api.rs"]
mod r#impl;

pub(crate) use self::r#impl::BIND;

mod ws;

mod image_cache;
use self::image_cache::ImageCache;

mod error;
use self::error::AppError;

use std::collections::HashSet;
use std::future::Future;
use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::{anyhow, Context, Error, Result};
use api::broadcast::{DashboardEpisode, DashboardSeries, DashboardUpdate};
use api::{ImageHint, ImageV2};
use axum::error_handling::HandleError;
use axum::extract::rejection::JsonRejection;
use axum::extract::{FromRequest, Path, Query};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Extension, Router};
use axum_extra::headers::ContentType;
use axum_extra::TypedHeader;
use derive_more::From;
use image::ImageFormat;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tower_http::cors::{AllowMethods, AllowOrigin, CorsLayer};

use crate::api::{themoviedb, thetvdb};
use crate::assets::ImageKey;
use crate::cache;
use crate::service::{paths, Service};

pub(crate) fn setup(
    listener: TcpListener,
    service: Service,
) -> Result<impl Future<Output = Result<()>>> {
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::any())
        .allow_methods(AllowMethods::any());

    let cache = ImageCache::new(
        service.paths.clone(),
        service.tvdb.clone(),
        service.tmdb.clone(),
    );

    let service = Arc::new(RwLock::new(service));

    let app = self::r#impl::router()
        .layer(Extension(service))
        .layer(Extension(cache))
        .layer(cors);

    let service = axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    );

    Ok(async move {
        service.await?;
        Ok(())
    })
}

fn common_routes(mut router: Router) -> Router {
    router = router.route("/ws", get(ws::entry));
    router = router.route("/api/graphics/*path", get(graphics));
    router
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Hint {
    Fit,
    Fill,
    #[default]
    Raw,
}

#[derive(Deserialize)]
struct GraphicsQuery {
    #[serde(default)]
    width: Option<u32>,
    #[serde(default)]
    height: Option<u32>,
    #[serde(default)]
    hint: Hint,
}

async fn graphics(
    Extension(cache): Extension<ImageCache>,
    Path(path): Path<String>,
    Query(params): Query<GraphicsQuery>,
) -> Result<(TypedHeader<ContentType>, Vec<u8>), AppError> {
    let hint = match (params.hint, params.width, params.height) {
        (Hint::Fit, Some(width), Some(height)) => ImageHint::Fit(width, height),
        (Hint::Fill, Some(width), Some(height)) => ImageHint::Fill(width, height),
        _ => ImageHint::Raw,
    };

    let image: ImageV2 = path.parse().map_err(Error::from)?;
    let (data, format) = cache.load_image(image, hint).await?;

    match format {
        ImageFormat::Jpeg => Ok((TypedHeader(ContentType::jpeg()), data)),
        _ => Ok((TypedHeader(ContentType::octet_stream()), data)),
    }
}

fn dashboard_update(service: &Service) -> DashboardUpdate {
    let mut schedule = Vec::with_capacity(service.schedule().len());
    let mut series_ids = HashSet::new();
    let mut episode_ids = HashSet::new();
    let mut series = Vec::new();
    let mut episodes = Vec::new();

    for day in service.schedule() {
        schedule.push(day.clone());

        for ss in &day.schedule {
            if series_ids.insert(ss.series_id) {
                if let Some(s) = service.series(&ss.series_id) {
                    series.push(s.clone());
                }
            }

            for id in &ss.episodes {
                if episode_ids.insert(*id) {
                    if let Some(e) = service.episode(id) {
                        episodes.push(e.episode().clone());
                    }
                }
            }
        }
    }

    DashboardUpdate {
        config: service.config().clone(),
        schedule: service.schedule().to_vec(),
        series,
        episodes,
    }
}
