mod disk;
mod source;

use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use tokio::time::MissedTickBehavior;

use crate::http::state::AppState;

pub use disk::DiskIndexSource;
pub use source::IndexSource;

const DEFAULT_RELOAD_INTERVAL_SECS: u64 = 60;

pub fn spawn_hot_reload(state: Arc<AppState>, source: impl IndexSource + 'static) {
    let interval_secs = std::env::var("INDEX_RELOAD_INTERVAL_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_RELOAD_INTERVAL_SECS);

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
        interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
        interval.tick().await;

        loop {
            interval.tick().await;
            if let Err(e) = reload_tick(&state, &source).await {
                eprintln!("index hot-reload tick failed: {e:#}");
            }
        }
    });
}

async fn reload_tick(
    state: &AppState,
    source: &(impl IndexSource + Clone + 'static),
) -> Result<()> {
    let disk_built_at = source.read_built_at_secs()?;
    let current = state.current_built_at_secs();
    if disk_built_at <= current {
        return Ok(());
    }

    eprintln!(
        "index hot-reload starting: built_at_secs {current} -> {disk_built_at} (loading from disk...)"
    );
    let started = Instant::now();

    let source = source.clone();
    let new_index = tokio::task::spawn_blocking(move || source.load_index())
        .await??;

    let elapsed = started.elapsed();

    match state.swap_if_newer(Arc::new(new_index)) {
        Some((old_secs, new_secs)) => {
            eprintln!(
                "index hot-reloaded: built_at_secs {old_secs} -> {new_secs} (loaded in {:.2}s)",
                elapsed.as_secs_f64()
            );
        }
        None => {
            eprintln!(
                "index hot-reload skipped after load: built_at_secs still {current} (loaded in {:.2}s, another swap won)",
                elapsed.as_secs_f64()
            );
        }
    }

    Ok(())
}
