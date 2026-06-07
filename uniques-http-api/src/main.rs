use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Context;
use tokio::net::TcpListener;
use uniques_http_api::{app, load_env, load_index, spawn_hot_reload, DiskIndexSource};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    load_env();

    let index_path = std::env::var("INDEX_PATH")
        .context("INDEX_PATH must point at the merged index directory (e.g. .../ALL_SETS)")?;

    let state = Arc::new(load_index(Path::new(&index_path))?);
    // spawn_hot_reload(
    //     Arc::clone(&state),
    //     DiskIndexSource::new(PathBuf::from(&index_path)),
    // );
    let app = app(state);

    let port: u16 = std::env::var("PORT")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .map(|v| v.parse::<u16>().context("PORT must be a valid u16 integer"))
        .transpose()?
        .unwrap_or(8080);

    let addr = format!("0.0.0.0:{port}");
    let listener = TcpListener::bind(&addr).await?;
    println!("Server started successfully at {addr}");
    axum::serve(listener, app).await?;
    Ok(())
}
