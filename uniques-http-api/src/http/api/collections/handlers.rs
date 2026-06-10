use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use serde::Serialize;

use crate::collections::{build_collection_bitmap, parse_refs_body, validate_collection_id};
use crate::http::api::error::{bad_request, ApiResult};
use crate::http::ServerState;

#[derive(Debug, Serialize)]
pub struct PostCollectionResponse {
    pub collection: String,
    pub count: u64,
}

pub async fn post_collection(
    State(server): State<ServerState>,
    Path(collection_id): Path<String>,
    body: String,
) -> ApiResult<Json<PostCollectionResponse>> {
    validate_collection_id(&collection_id).map_err(bad_request)?;

    let refs = parse_refs_body(&body);
    if refs.is_empty() {
        return Err(bad_request("collection must contain at least one reference"));
    }

    let snapshot = server.app.snapshot();
    let index = snapshot.index.as_ref();
    let (count, bitmap) = build_collection_bitmap(
        index.catalog(),
        &refs,
        index.manifest().total_bit_span,
    )
    .map_err(|e| bad_request(e.to_string()))?;

    snapshot
        .collections
        .insert(&collection_id, Arc::new(bitmap));

    Ok(Json(PostCollectionResponse {
        collection: collection_id,
        count,
    }))
}
