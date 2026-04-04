//! User session management.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// User session data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSession {
    /// User ID.
    pub user_id: String,
    /// Username.
    pub username: String,
    /// Email.
    pub email: String,
    /// Display name.
    pub display_name: String,
    /// Authentication token (stub).
    #[serde(skip_serializing, skip_deserializing, default)]
    pub token: String,
    /// Last login time.
    pub last_login: String,
    /// Whether the user is authenticated.
    pub is_authenticated: bool,
    /// Recent projects.
    pub recent_projects: Vec<RecentProject>,
}

/// Recent project entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentProject {
    pub name: String,
    pub path: String,
    pub last_opened: String,
}

impl UserSession {
    /// Create a new guest session.
    pub fn guest() -> Self {
        Self {
            user_id: "guest".to_string(),
            username: "guest".to_string(),
            email: String::new(),
            display_name: "Guest User".to_string(),
            token: String::new(),
            last_login: chrono::Utc::now().to_rfc3339(),
            is_authenticated: false,
            recent_projects: Vec::new(),
        }
    }

    /// Create a new authenticated session (stub).
    pub fn authenticated(username: &str, email: &str) -> Self {
        Self {
            user_id: format!("user_{}", uuid::Uuid::new_v4()),
            username: username.to_string(),
            email: email.to_string(),
            display_name: username.to_string(),
            token: format!("token_{}", uuid::Uuid::new_v4()),
            last_login: chrono::Utc::now().to_rfc3339(),
            is_authenticated: true,
            recent_projects: Vec::new(),
        }
    }

    /// Get the session file path.
    fn session_file_path() -> PathBuf {
        let config_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("snappix");

        std::fs::create_dir_all(&config_dir).ok();
        config_dir.join("session.json")
    }

    /// Save session to disk.
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = Self::session_file_path();
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Load session from disk.
    pub fn load() -> Option<Self> {
        let path = Self::session_file_path();
        if path.exists() {
            let content = std::fs::read_to_string(path).ok()?;
            serde_json::from_str(&content).ok()
        } else {
            None
        }
    }

    /// Clear session (logout).
    pub fn clear() {
        let path = Self::session_file_path();
        if path.exists() {
            std::fs::remove_file(path).ok();
        }
    }

    /// Logout the user.
    pub fn logout(&mut self) {
        self.is_authenticated = false;
        self.token.clear();
        Self::clear();
    }
}

impl Default for UserSession {
    fn default() -> Self {
        Self::load().unwrap_or_else(Self::guest)
    }
}
