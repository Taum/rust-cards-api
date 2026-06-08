use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Settings {
    pub server: ServerSettings,
    pub index: IndexSettings,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerSettings {
    pub port: u16,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IndexSettings {
    pub source: IndexSourceKind,
    #[serde(default)]
    pub path: Option<String>,
    pub reload: ReloadSettings,
    #[serde(default)]
    pub object_store: ObjectStoreSettings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IndexSourceKind {
    Disk,
    Archive,
    ObjectStore,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReloadSettings {
    pub enabled: bool,
    #[serde(default)]
    pub interval_secs: Option<u64>,
}

impl ReloadSettings {
    /// Validated interval for hot-reload. Only call when `enabled` is true.
    pub fn interval_secs(&self) -> Result<u64> {
        validate_reload(self)?;
        self.interval_secs
            .context("index.reload.interval_secs missing after validation")
    }
}

fn validate_reload(reload: &ReloadSettings) -> Result<()> {
    match (reload.enabled, reload.interval_secs) {
        (true, None) => bail!(
            "index.reload.interval_secs is required when index.reload.enabled = true"
        ),
        (false, Some(_)) => bail!(
            "index.reload.interval_secs must not be set when index.reload.enabled = false"
        ),
        _ => Ok(()),
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ObjectStoreSettings {
    #[serde(default)]
    pub url: String,
}

impl Settings {
    pub fn index_path(&self) -> Result<PathBuf> {
        let path = self
            .index
            .path
            .as_deref()
            .filter(|p| !p.trim().is_empty())
            .context(
                "index.path must be set for disk/archive sources (config or INDEX_PATH env)",
            )?;
        Ok(PathBuf::from(path))
    }

    pub fn object_store_url(&self) -> Result<&str> {
        let url = self.index.object_store.url.trim();
        if url.is_empty() {
            bail!("index.object_store.url must be set when index.source = object_store");
        }
        Ok(url)
    }
}

pub fn load_settings() -> Result<Settings> {
    let config_dir = config_dir();
    let app_env = std::env::var("APP_ENV").unwrap_or_else(|_| "local".to_string());

    let mut settings: Settings = config::Config::builder()
        .add_source(config::File::from(config_dir.join("default.toml")))
        .add_source(
            config::File::from(config_dir.join(format!("{app_env}.toml"))).required(false),
        )
        .add_source(config::Environment::default().separator("__"))
        .build()
        .context("build config")?
        .try_deserialize()
        .context("deserialize config")?;

    apply_legacy_env_overrides(&mut settings);
    validate_settings(&settings)?;
    Ok(settings)
}

fn config_dir() -> PathBuf {
    std::env::var("CONFIG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| Path::new(env!("CARGO_MANIFEST_DIR")).join("config"))
}

fn apply_legacy_env_overrides(settings: &mut Settings) {
    if let Ok(path) = std::env::var("INDEX_PATH") {
        if !path.trim().is_empty() {
            eprintln!("note: INDEX_PATH env override; prefer index.path in config");
            settings.index.path = Some(path);
        }
    }

    if let Ok(port) = std::env::var("PORT") {
        if let Ok(port) = port.trim().parse::<u16>() {
            settings.server.port = port;
        }
    }

    if let Ok(secs) = std::env::var("INDEX_RELOAD_INTERVAL_SECS") {
        if let Ok(secs) = secs.trim().parse::<u64>() {
            settings.index.reload.interval_secs = Some(secs);
        }
    }
}

fn validate_settings(settings: &Settings) -> Result<()> {
    match settings.index.source {
        IndexSourceKind::ObjectStore => {
            settings.object_store_url()?;
        }
        IndexSourceKind::Disk | IndexSourceKind::Archive => {
            settings.index_path()?;
        }
    }
    validate_reload(&settings.index.reload)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_settings_deserialize_from_files() {
        let settings = load_settings().expect("load settings");
        assert_eq!(settings.server.port, 8234);
        assert_eq!(settings.index.source, IndexSourceKind::Disk);
        assert!(!settings.index.reload.enabled);
        assert!(settings.index.reload.interval_secs.is_none());
        assert!(settings.index.object_store.url.is_empty());
    }

    #[test]
    fn disk_source_does_not_require_object_store_section() {
        let toml = r#"
            [server]
            port = 3000

            [index]
            source = "disk"
            path = "./index"

            [index.reload]
            enabled = false
        "#;
        let settings: Settings = config::Config::builder()
            .add_source(config::File::from_str(toml, config::FileFormat::Toml))
            .build()
            .unwrap()
            .try_deserialize()
            .unwrap();
        assert_eq!(settings.index.source, IndexSourceKind::Disk);
        validate_settings(&settings).expect("disk config validates without object_store");
    }

    #[test]
    fn reload_interval_required_when_enabled() {
        let reload = ReloadSettings {
            enabled: true,
            interval_secs: None,
        };
        assert!(validate_reload(&reload).is_err());
    }

    #[test]
    fn reload_interval_forbidden_when_disabled() {
        let reload = ReloadSettings {
            enabled: false,
            interval_secs: Some(60),
        };
        assert!(validate_reload(&reload).is_err());
    }

    #[test]
    fn reload_interval_ok_when_enabled_and_set() {
        let reload = ReloadSettings {
            enabled: true,
            interval_secs: Some(30),
        };
        assert_eq!(validate_reload(&reload).unwrap(), ());
        assert_eq!(reload.interval_secs().unwrap(), 30);
    }
}
