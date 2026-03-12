use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use crate::models::Project;

pub struct JsonStore;

/// Returns `~/.config/wifichecker/` (XDG-konform).
pub fn config_dir() -> PathBuf {
    let base = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".to_string()))
                .join(".config")
        });
    base.join("wifichecker")
}

/// Returns `~/.config/wifichecker/drawings/`.
pub fn drawings_dir() -> PathBuf {
    config_dir().join("drawings")
}

/// Ensures config dir and drawings subdir exist.
pub fn ensure_config_dirs() -> Result<()> {
    std::fs::create_dir_all(drawings_dir())
        .context("Failed to create ~/.config/wifichecker/drawings/")?;
    Ok(())
}

impl JsonStore {
    pub fn save(project: &Project, path: &Path) -> Result<()> {
        let json = serde_json::to_string_pretty(project)
            .context("Failed to serialize project")?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        std::fs::write(path, json)
            .with_context(|| format!("Failed to write project to {}", path.display()))?;
        Ok(())
    }

    pub fn load(path: &Path) -> Result<Project> {
        let json = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read project from {}", path.display()))?;
        let project = serde_json::from_str(&json)
            .context("Failed to deserialize project")?;
        Ok(project)
    }

    /// Default project path: `~/.config/wifichecker/project.json`
    pub fn default_path() -> PathBuf {
        config_dir().join("project.json")
    }
}
