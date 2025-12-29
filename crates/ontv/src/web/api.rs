use std::sync::Arc;

use anyhow::Context;
use api::{ImageHint, ImageV2};
use axum::extract::Path;
use axum::http::header;
use axum::response::{Html, IntoResponse};
use axum::{Extension, Router};
use tokio::fs;
use tokio::sync::RwLock;

use crate::assets::ImageKey;
use crate::Service;

use super::WebError;

pub(super) async fn image(
    Extension(service): Extension<Arc<RwLock<Service>>>,
    Path((hint, image)): Path<(ImageHint, ImageV2)>,
) -> Result<impl IntoResponse, WebError> {
    let service = service.read().await;
    let html = format!("{hint:?}/{image:?}");
    let path = service.load_image(image, hint).await?;

    let Some(mime) = mime_guess::from_path(&path).first() else {
        return Err(WebError::not_found());
    };

    let bytes = fs::read(&path).await.context("reading image file")?;
    let response = ([(header::CONTENT_TYPE, mime.as_ref())], bytes).into_response();
    Ok(response)
}
