use std::sync::Arc;

use anyhow::Context;
use tokio::net::TcpListener;
use uniques_http_api::{
    app, load_env, load_index, load_index_from_object_store, load_settings,
    spawn_hot_reload, AnyIndexSource, DiskIndexSource, IndexSourceKind,
    ObjectStoreIndexClient, RemoteIndexSource, Settings,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    load_env();
    let settings = load_settings()?;

    let (state, reload_source) = load_app_state(&settings)?;
    let state = Arc::new(state);

    if settings.index.reload.enabled {
        if let Some(source) = reload_source {
            spawn_hot_reload(
                Arc::clone(&state),
                source,
                settings.index.reload.interval_secs()?,
            );
        }
    }

    let app = app(state);
    let addr = format!("0.0.0.0:{}", settings.server.port);
    let listener = TcpListener::bind(&addr).await?;
    println!("Server started successfully at {addr}");
    axum::serve(listener, app).await?;
    Ok(())
}

fn load_app_state(
    settings: &Settings,
) -> anyhow::Result<(uniques_http_api::AppState, Option<AnyIndexSource>)> {
    match settings.index.source {
        IndexSourceKind::Disk | IndexSourceKind::Archive => {
            let path = settings.index_path()?;
            let reload_source = settings
                .index
                .reload
                .enabled
                .then(|| AnyIndexSource::Disk(DiskIndexSource::new(path.clone())));
            Ok((load_index(&path)?, reload_source))
        }
        IndexSourceKind::ObjectStore => {
            let url = settings.object_store_url()?.to_string();
            let client = ObjectStoreIndexClient::new(&url)
                .with_context(|| format!("connect to object store at {url}"))?;
            let reload_source = settings
                .index
                .reload
                .enabled
                .then(|| AnyIndexSource::Remote(RemoteIndexSource::new(client.clone())));
            Ok((
                load_index_from_object_store(&client)?,
                reload_source,
            ))
        }
    }
}
