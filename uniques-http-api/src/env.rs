use std::path::Path;

/// Load `.env`, then `.env.local` from the crate directory (local overrides shared defaults).
pub fn load_env() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"));

    let _ = dotenvy::from_path(dir.join(".env")).ok();
    let _ = dotenvy::from_path_override(dir.join(".env.local")).ok();
}
