use std::path::Path;
use std::sync::Arc;

use anyhow::Context;
use tokio::net::TcpListener;
use uniques_http_api::{app, load_env, load_index};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    load_env();

    let index_path = std::env::var("INDEX_PATH")
        .context("INDEX_PATH must point at the merged index directory (e.g. .../ALL_SETS)")?;

    let state = Arc::new(load_index(Path::new(&index_path))?);
    let app = app(state);

    let listener = TcpListener::bind("0.0.0.0:8234").await?;
    println!("Server started successfully at 0.0.0.0:8234");
    axum::serve(listener, app).await?;
    Ok(())
}
