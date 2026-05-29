use std::path::Path;
use std::sync::Arc;

use axum::body::Body;
use http_body_util::BodyExt;
use tower::ServiceExt;
use uniques_http_api::{app, load_index};

const FIXTURE_INDEX: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/minimal_index");

fn test_state() -> Arc<uniques_http_api::AppState> {
    Arc::new(
        load_index(Path::new(FIXTURE_INDEX)).expect("load minimal test index"),
    )
}

#[tokio::test]
async fn cards_omit_bga_debug_trigram_by_default() {
    let response = app(test_state())
        .oneshot(
            axum::http::Request::builder()
                .uri("/api/v2/cards")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(value["cards"][0].get("debug_bga_trigram").is_none());
}

#[tokio::test]
async fn cards_include_bga_debug_trigram_when_requested() {
    let response = app(test_state())
        .oneshot(
            axum::http::Request::builder()
                .uri("/api/v2/cards?debug_bga_trigram")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(value["cards"][0]["debug_bga_trigram"].is_string());
}
