use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;

use crate::http::state::AppState;

use super::source::IndexSource;

pub(super) async fn reload_tick(
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
