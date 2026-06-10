use axum::http::StatusCode;
use axum::Json;
use serde_json::{json, Value};

use crate::index::QueryError;

pub(crate) type ApiResult<T> = Result<T, (StatusCode, Json<Value>)>;

pub(crate) fn bad_request(message: impl Into<String>) -> (StatusCode, Json<Value>) {
    (
        StatusCode::BAD_REQUEST,
        Json(json!({ "error": message.into() })),
    )
}

pub(crate) fn not_found(message: impl Into<String>) -> (StatusCode, Json<Value>) {
    (
        StatusCode::NOT_FOUND,
        Json(json!({ "error": message.into() })),
    )
}

pub(crate) fn map_query_error(err: QueryError) -> (StatusCode, Json<Value>) {
    bad_request(err.message())
}

pub(crate) fn internal_server_error(message: impl Into<String>) -> (StatusCode, Json<Value>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "error": message.into() })),
    )
}

pub(crate) fn collection_not_loaded(collection_id: String) -> (StatusCode, Json<Value>) {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(json!({
            "error": "collection_not_loaded",
            "collection": collection_id,
        })),
    )
}
