use axum::extract::rejection::JsonRejection;
use axum::extract::FromRequest;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use derive_more::From;
use serde::Serialize;

#[derive(Serialize)]
struct ErrorResponse {
    message: String,
}

#[derive(From)]
pub(super) enum AppError {
    Error(anyhow::Error),
    JsonRejection(JsonRejection),
}

#[derive(FromRequest)]
#[from_request(via(axum::Json), rejection(AppError))]
pub(super) struct AppJson<T>(T);

impl<T> IntoResponse for AppJson<T>
where
    T: Serialize,
{
    fn into_response(self) -> Response {
        axum::Json(self.0).into_response()
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match self {
            AppError::JsonRejection(rejection) => {
                let status = rejection.status();

                let error = ErrorResponse {
                    message: rejection.body_text(),
                };

                (status, AppJson(error)).into_response()
            }
            AppError::Error(error) => {
                let status = StatusCode::INTERNAL_SERVER_ERROR;

                let error = ErrorResponse {
                    message: error.to_string(),
                };

                (status, AppJson(error)).into_response()
            }
        }
    }
}
