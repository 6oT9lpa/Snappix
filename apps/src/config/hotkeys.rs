//! Hotkeys configuration.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::app_errors::{self, AppErrorCode};

const APP_CONFIG_DIR: &str = "snappix";
const HOTKEYS_BINARY_FILE: &str = "hotkeys.bin";

/// Hotkeys configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeysConfig {
    pub version: String,
    pub description: String,
    pub hotkeys: HashMap<String, HashMap<String, HotkeyAction>>,
}

/// Single hotkey action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeyAction {
    pub key: String,
    pub description: String,
}

impl HotkeysConfig {
    /// Load hotkeys from file.
    #[allow(dead_code)]
    pub fn load(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let config: HotkeysConfig = serde_json::from_str(&content)?;
        Ok(config)
    }

    /// Resolve binary config path in AppData/config dir.
    pub fn appdata_binary_path() -> PathBuf {
        let config_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(APP_CONFIG_DIR);
        config_dir.join(HOTKEYS_BINARY_FILE)
    }

    /// Load hotkeys from MessagePack binary file.
    pub fn load_binary(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let bytes = std::fs::read(path)?;
        let config: HotkeysConfig =
            shared::from_msgpack(&bytes).map_err(|err| std::io::Error::other(err.to_string()))?;
        Ok(config)
    }

    /// Save hotkeys into MessagePack binary file.
    pub fn save_binary(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let bytes =
            shared::to_msgpack(self).map_err(|err| std::io::Error::other(err.to_string()))?;
        std::fs::write(path, bytes)?;
        Ok(())
    }

    /// Load AppData config or initialize it from defaults.
    pub fn load_or_initialize() -> Self {
        let path = Self::appdata_binary_path();

        if path.exists() {
            match Self::load_binary(&path) {
                Ok(config) => return config,
                Err(err) => {
                    eprintln!(
                        "Failed to load hotkeys from {}: {err}. Recreating from defaults.",
                        path.display()
                    );
                    app_errors::report(AppErrorCode::HotkeysLoadFailed);
                }
            }
        }

        let config = Self::load_default();
        if let Err(err) = config.save_binary(&path) {
            eprintln!(
                "Failed to write hotkeys config to {}: {err}",
                path.display()
            );
            app_errors::report(AppErrorCode::HotkeysWriteFailed);
        }
        config
    }

    /// Persist current config into AppData binary file.
    pub fn save_to_appdata(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.save_binary(&Self::appdata_binary_path())
    }

    /// Load default hotkeys.
    pub fn load_default() -> Self {
        let default_json = include_str!("../../config/hotkeys.json");
        serde_json::from_str(default_json).expect("Failed to parse default hotkeys")
    }

    /// Get hotkey by category and action name.
    pub fn get(&self, category: &str, action: &str) -> Option<&HotkeyAction> {
        self.hotkeys.get(category)?.get(action)
    }

    /// Get key string for a hotkey.
    pub fn get_key(&self, category: &str, action: &str) -> Option<&str> {
        self.get(category, action).map(|h| h.key.as_str())
    }

    /// Set or update a hotkey action.
    #[allow(dead_code)]
    pub fn set_hotkey(
        &mut self,
        category: &str,
        action: &str,
        key: impl Into<String>,
        description: impl Into<String>,
    ) {
        let category_map = self.hotkeys.entry(category.to_string()).or_default();
        category_map.insert(
            action.to_string(),
            HotkeyAction {
                key: key.into(),
                description: description.into(),
            },
        );
    }
}

impl Default for HotkeysConfig {
    fn default() -> Self {
        Self::load_default()
    }
}

/// Parse key combination string into components.
pub fn parse_key_combo(combo: &str) -> KeyCombo {
    let parts: Vec<&str> = combo.split('+').collect();
    let mut key_combo = KeyCombo::default();

    for part in parts {
        let part = part.trim();
        match part.to_lowercase().as_str() {
            "ctrl" => key_combo.ctrl = true,
            "shift" => key_combo.shift = true,
            "alt" => key_combo.alt = true,
            "mousewheel" => key_combo.mouse_wheel = true,
            _ => key_combo.key = part.to_string(),
        }
    }

    key_combo
}

/// Key combination.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct KeyCombo {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub key: String,
    pub mouse_wheel: bool,
}
