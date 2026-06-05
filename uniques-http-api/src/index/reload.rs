mod disk;
mod source;
mod tick;

use std::sync::Arc;
use std::time::Duration;

use crate::http::state::AppState;
use tokio::time::MissedTickBehavior;

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
            if let Err(e) = tick::reload_tick(&state, &source).await {
                eprintln!("index hot-reload tick failed: {e:#}");
            }
        }
    });
}
