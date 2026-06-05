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
async fn effects_returns_memoized_catalog_json() {
    let response = app(test_state())
        .oneshot(
            axum::http::Request::builder()
                .uri("/api/v2/effects")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    assert_eq!(
        response.headers().get("content-type").and_then(|v| v.to_str().ok()),
        Some("application/json")
    );

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(value["triggers"].as_array().unwrap().len(), 1);
    assert_eq!(value["triggers"][0]["idGd"], 1);
    assert_eq!(value["triggers"][0]["text"]["en_US"], "{R}");
    assert_eq!(value["conditions"].as_array().unwrap().len(), 1);
    assert_eq!(value["output"].as_array().unwrap().len(), 1);
    assert_eq!(value["output"][0]["idGd"], 3);

    // Same bytes as startup memoization (stable across requests).
    let state = test_state();
    assert_eq!(body.as_ref(), state.index().effects_body().as_ref());
}
