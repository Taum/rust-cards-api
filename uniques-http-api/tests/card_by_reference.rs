use std::path::Path;

use axum::body::Body;
use http_body_util::BodyExt;
use tower::ServiceExt;
use uniques_http_api::{app, load_index, ServerState};

const FIXTURE_INDEX: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/minimal_index");

fn test_server() -> ServerState {
    ServerState::for_test(
        load_index(Path::new(FIXTURE_INDEX)).expect("load minimal test index"),
    )
}

#[tokio::test]
async fn card_by_reference_returns_card_v2() {
    let response = app(test_server())
        .oneshot(
            axum::http::Request::builder()
                .uri("/api/v2/card/ALT_TEST_B_AX_04_U_1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let card: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(card["reference"], "ALT_TEST_B_AX_04_U_1");
    assert_eq!(card["name"]["en_US"], "Fixture Card");
    assert_eq!(card["artist"], "Fixture Artist");
    assert_eq!(card["set"]["code"], "BTG");
    assert_eq!(card["cardSubTypes"][0]["reference"], "ENGINEER");
    assert!(card.get("cards").is_none());
    assert!(card.get("iter").is_none());
}

#[tokio::test]
async fn card_by_reference_rejects_invalid_reference() {
    let response = app(test_server())
        .oneshot(
            axum::http::Request::builder()
                .uri("/api/v2/card/not-a-ref")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 400);
}

#[tokio::test]
async fn card_by_reference_returns_404_for_unknown_family() {
    let response = app(test_server())
        .oneshot(
            axum::http::Request::builder()
                .uri("/api/v2/card/ALT_TEST_B_AX_99_U_1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), 404);
}
