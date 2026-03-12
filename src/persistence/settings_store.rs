use anyhow::{Context, Result};
use crate::models::AppSettings;
use crate::persistence::json_store::config_dir;

pub struct SettingsStore;

impl SettingsStore {
    fn path() -> std::path::PathBuf {
        config_dir().join("settings.json")
    }

    pub fn load() -> AppSettings {
        let path = Self::path();
        if !path.exists() {
            return AppSettings::default();
        }
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(settings: &AppSettings) -> Result<()> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let json = serde_json::to_string_pretty(settings)
            .context("Failed to serialize settings")?;
        std::fs::write(&path, json)
            .context("Failed to write settings.json")?;
        Ok(())
    }
}
