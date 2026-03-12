use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use crate::models::Project;

pub struct JsonStore;

impl JsonStore {
    pub fn save(project: &Project, path: &Path) -> Result<()> {
        let json = serde_json::to_string_pretty(project)
            .context("Failed to serialize project")?;
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

    pub fn default_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join("wifichecker_project.json")
    }
}
