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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Floor, Project};

    fn temp_path(suffix: &str) -> PathBuf {
        std::env::temp_dir().join(format!("wifichecker_test_{}_{}.json", std::process::id(), suffix))
    }

    #[test]
    fn test_config_dir_ends_with_wifichecker() {
        let dir = config_dir();
        assert!(dir.ends_with("wifichecker"));
    }

    #[test]
    fn test_drawings_dir_ends_with_drawings() {
        let dir = drawings_dir();
        assert!(dir.ends_with("drawings"));
    }

    #[test]
    fn test_default_path_filename() {
        let path = JsonStore::default_path();
        assert_eq!(path.file_name().unwrap(), "project.json");
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let mut project = Project::new("Test Project");
        project.add_floor(Floor::new("Ground Floor"));
        project.add_floor(Floor::new("First Floor"));

        let path = temp_path("roundtrip");
        JsonStore::save(&project, &path).expect("save failed");

        let loaded = JsonStore::load(&path).expect("load failed");
        assert_eq!(loaded.name, "Test Project");
        assert_eq!(loaded.floors.len(), 2);
        assert_eq!(loaded.floors[0].name, "Ground Floor");
        assert_eq!(loaded.floors[1].name, "First Floor");

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_save_creates_parent_directory() {
        let dir = std::env::temp_dir().join(format!("wifichecker_test_dir_{}", std::process::id()));
        let path = dir.join("nested").join("project.json");

        let project = Project::new("Nested");
        JsonStore::save(&project, &path).expect("save with nested dir failed");
        assert!(path.exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_load_nonexistent_file_returns_error() {
        let path = std::path::Path::new("/tmp/wifichecker_nonexistent_abc123.json");
        assert!(JsonStore::load(path).is_err());
    }

    #[test]
    fn test_load_invalid_json_returns_error() {
        let path = temp_path("invalid_json");
        std::fs::write(&path, "this is not valid json").unwrap();
        assert!(JsonStore::load(&path).is_err());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_save_and_load_empty_project() {
        let project = Project::new("Empty");
        let path = temp_path("empty");
        JsonStore::save(&project, &path).unwrap();
        let loaded = JsonStore::load(&path).unwrap();
        assert_eq!(loaded.name, "Empty");
        assert!(loaded.floors.is_empty());
        let _ = std::fs::remove_file(&path);
    }
}
