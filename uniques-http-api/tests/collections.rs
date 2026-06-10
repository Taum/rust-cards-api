use std::path::Path;

use axum::body::Body;
use http_body_util::BodyExt;
use tower::ServiceExt;
use uniques_http_api::{app, load_index, ServerState};

const FIXTURE_INDEX: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/minimal_index");
const FIXTURE_REF: &str = "ALT_TEST_B_AX_04_U_1";

fn test_server() -> ServerState {
    ServerState::for_test(
        load_index(Path::new(FIXTURE_INDEX)).expect("load minimal test index"),
    )
}

#[tokio::test]
async fn post_collection_then_query_filters_cards() {
    let server = test_server();
    let collection_id = "test-deck";

    let post_response = app(server.clone())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri(format!("/api/v2/collection/{collection_id}"))
                .header("content-type", "text/plain")
                .body(Body::from(format!("{FIXTURE_REF}\n")))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(post_response.status(), 200);
    let post_body = post_response.into_body().collect().await.unwrap().to_bytes();
    let posted: serde_json::Value = serde_json::from_slice(&post_body).unwrap();
    assert_eq!(posted["collection"], collection_id);
    assert_eq!(posted["count"], 1);

    let query_response = app(server)
        .oneshot(
            axum::http::Request::builder()
                .uri(format!("/api/v2/cards?collection={collection_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(query_response.status(), 200);
    let query_body = query_response.into_body().collect().await.unwrap().to_bytes();
    let cards: serde_json::Value = serde_json::from_slice(&query_body).unwrap();
    assert_eq!(cards["iter"]["total"], 1);
    assert_eq!(cards["cards"][0]["reference"], FIXTURE_REF);
}

#[tokio::test]
async fn query_unknown_collection_returns_422() {
    let response = app(test_server())
        .oneshot(
            axum::http::Request::builder()
                .uri("/api/v2/cards?collection=missing-deck")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 422);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let err: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(err["error"], "collection_not_loaded");
    assert_eq!(err["collection"], "missing-deck");
}

#[tokio::test]
async fn post_collection_rejects_invalid_reference() {
    let response = app(test_server())
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/v2/collection/test-deck")
                .header("content-type", "text/plain")
                .body(Body::from("not-a-ref\n"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 400);
}
