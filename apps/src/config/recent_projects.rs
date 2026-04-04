//! Recent projects storage.
//!
//! Manages a persistent list of recently opened projects.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Recent project entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentProject {
    pub name: String,
    pub path: String,
    pub last_opened: String,
}

/// Recent projects storage
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RecentProjectsStorage {
    pub projects: Vec<RecentProject>,
}

impl RecentProjectsStorage {
    /// Get the storage file path
    fn storage_file_path() -> PathBuf {
        let config_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("snappix");

        std::fs::create_dir_all(&config_dir).ok();
        config_dir.join("recent_projects.json")
    }

    /// Load recent projects from disk
    pub fn load() -> Self {
        let path = Self::storage_file_path();
        if path.exists() {
            let content = std::fs::read_to_string(path).unwrap_or_default();
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            Self::default()
        }
    }

    /// Save recent projects to disk
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = Self::storage_file_path();
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Add a project to the recent list
    pub fn add_project(&mut self, name: &str, path: &str) {
        // Remove if already exists
        self.projects.retain(|p| p.path != path);

        // Add to front
        self.projects.insert(
            0,
            RecentProject {
                name: name.to_string(),
                path: path.to_string(),
                last_opened: chrono::Utc::now().to_rfc3339(),
            },
        );

        // Keep only last 10
        if self.projects.len() > 10 {
            self.projects.truncate(10);
        }

        // Save
        self.save().ok();
    }

    /// Remove a project from the recent list by path and/or name.
    pub fn remove_project_entry(&mut self, path: &str, name: &str) -> bool {
        let Some(index) = self.find_project_index(path, name) else {
            return false;
        };
        self.projects.remove(index);
        self.save().ok();
        true
    }

    /// Rename a project entry in recent list by path and/or name.
    pub fn rename_project_entry(&mut self, path: &str, name: &str, new_name: &str) -> bool {
        let trimmed = new_name.trim();
        if trimmed.is_empty() {
            return false;
        }

        let Some(index) = self.find_project_index(path, name) else {
            return false;
        };
        self.projects[index].name = trimmed.to_string();
        self.projects[index].last_opened = chrono::Utc::now().to_rfc3339();
        self.save().ok();
        true
    }

    /// Update storage path for a project entry by path and/or name.
    pub fn relocate_project_entry(&mut self, path: &str, name: &str, new_path: &str) -> bool {
        let trimmed = new_path.trim();
        if trimmed.is_empty() {
            return false;
        }

        let Some(index) = self.find_project_index(path, name) else {
            return false;
        };
        self.projects[index].path = trimmed.to_string();
        self.projects[index].last_opened = chrono::Utc::now().to_rfc3339();
        self.save().ok();
        true
    }

    /// Get all recent projects
    pub fn get_all(&self) -> &[RecentProject] {
        &self.projects
    }

    fn find_project_index(&self, path: &str, name: &str) -> Option<usize> {
        let path = path.trim();
        let name = name.trim();

        if !path.is_empty() {
            if let Some(index) = self
                .projects
                .iter()
                .position(|project| project.path == path)
            {
                return Some(index);
            }
        }

        if !name.is_empty() {
            if !path.is_empty() {
                if let Some(index) = self
                    .projects
                    .iter()
                    .position(|project| project.name == name && project.path == path)
                {
                    return Some(index);
                }
            }

            if let Some(index) = self
                .projects
                .iter()
                .position(|project| project.name == name)
            {
                return Some(index);
            }
        }

        None
    }
}
