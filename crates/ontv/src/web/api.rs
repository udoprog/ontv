use axum::Router;

pub(crate) static BIND: &str = "127.0.0.1:44614";

pub(crate) fn router() -> Router {
    super::common_routes(Router::new())
}
