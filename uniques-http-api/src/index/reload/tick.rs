use std::sync::Arc;

use crate::config::Settings;
use crate::formats::rebuild_formats_for_index;
use crate::http::state::AppState;

use super::source::IndexSource;

pub(super) async fn reload_tick(
    state: &AppState,
    settings: &Settings,
    source: &(impl IndexSource + Clone + 'static),
) -> anyhow::Result<()> {
    let remote_version = source.read_version()?;
    let current = state.current_built_at_secs();
    if remote_version <= current {
        return Ok(());
    }

    eprintln!(
        "index hot-reload starting: version {current} -> {remote_version} (loading index...)"
    );
    let started = std::time::Instant::now();

    let source = source.clone();
    let formats_settings = settings.formats.clone();
    let new_index = tokio::task::spawn_blocking(move || source.load_index())
        .await??;

    let new_index = Arc::new(new_index);
    let new_formats = rebuild_formats_for_index(
        Arc::clone(&new_index),
        formats_settings.as_ref().filter(|f| f.is_enabled()),
    );

    let elapsed = started.elapsed();

    match state.commit_if_newer(new_index, new_formats) {
        Some((old_secs, new_secs)) => {
            eprintln!(
                "index hot-reloaded: version {old_secs} -> {new_secs} (loaded in {:.2}s)",
                elapsed.as_secs_f64()
            );
        }
        None => {
            eprintln!(
                "index hot-reload skipped after load: version still {current} (loaded in {:.2}s, another swap won)",
                elapsed.as_secs_f64()
            );
        }
    }

    Ok(())
}
