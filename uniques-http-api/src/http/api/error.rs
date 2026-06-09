use axum::http::StatusCode;
use axum::Json;
use serde::Serialize;

use crate::index::QueryError;

#[derive(Debug, Serialize)]
pub struct ApiError {
    pub error: String,
}

pub(crate) type ApiResult<T> = Result<T, (StatusCode, Json<ApiError>)>;

pub(crate) fn bad_request(message: impl Into<String>) -> (StatusCode, Json<ApiError>) {
    (
        StatusCode::BAD_REQUEST,
        Json(ApiError {
            error: message.into(),
        }),
    )
}

pub(crate) fn not_found(message: impl Into<String>) -> (StatusCode, Json<ApiError>) {
    (
        StatusCode::NOT_FOUND,
        Json(ApiError {
            error: message.into(),
        }),
    )
}

pub(crate) fn map_query_error(err: QueryError) -> (StatusCode, Json<ApiError>) {
    bad_request(err.message())
}

pub(crate) fn internal_server_error(message: impl Into<String>) -> (StatusCode, Json<ApiError>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ApiError {
            error: message.into(),
        }),
    )
}
