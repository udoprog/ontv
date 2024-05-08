use std::borrow::Cow;

use axum::http::{header, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use rust_embed::RustEmbed;

pub(crate) static BIND: &str = "127.0.0.1:44614";

pub(crate) fn router() -> Router {
    let router = Router::new().route("/", get(index_handler));

    let router = super::common_routes(router);

    router
        .route("/*file", get(static_handler))
        .fallback(index_handler)
}

async fn index_handler() -> impl IntoResponse {
    StaticFile(Cow::Borrowed("index.html"))
}

async fn static_handler(uri: Uri) -> impl IntoResponse {
    StaticFile(Cow::Owned(uri.path().trim_start_matches('/').to_string()))
}

#[derive(RustEmbed)]
#[folder = "../web/dist"]
struct Asset;

pub struct StaticFile(Cow<'static, str>);

impl IntoResponse for StaticFile {
    fn into_response(self) -> Response {
        match Asset::get(self.0.as_ref()) {
            Some(content) => {
                let mime = mime_guess::from_path(self.0.as_ref()).first_or_octet_stream();
                ([(header::CONTENT_TYPE, mime.as_ref())], content.data).into_response()
            }
            None => (StatusCode::NOT_FOUND, "404 Not Found").into_response(),
        }
    }
}
