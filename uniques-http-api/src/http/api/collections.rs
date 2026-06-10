mod handlers;

use axum::extract::DefaultBodyLimit;
use axum::Router;

use crate::config::CollectionsSettings;
use crate::http::ServerState;

pub fn router(collections: &CollectionsSettings) -> Router<ServerState> {
    use axum::routing::post;

    let route = Router::new().route(
        "/api/v2/collection/{collection_id}",
        post(handlers::post_collection),
    );

    if collections.max_post_payload_bytes > 0 {
        let limit = collections.max_post_payload_bytes as usize;
        route.layer(DefaultBodyLimit::max(limit))
    } else {
        route
    }
}
