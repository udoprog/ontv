use axum::Router;

use super::common_routes;

pub(crate) fn router() -> Router {
    common_routes(Router::new())
}
