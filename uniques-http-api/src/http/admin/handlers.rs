use axum::Json;
use serde::Serialize;

#[derive(Serialize)]
pub struct HelloResponse {
    pub message: &'static str,
}

pub async fn healthz() -> Json<HelloResponse> {
    Json(HelloResponse {
        message: "Hello World",
    })
}
