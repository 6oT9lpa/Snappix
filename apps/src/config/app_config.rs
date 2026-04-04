//! Global editor configuration (language/theme/hotkeys/pointer overlay).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::app_errors::{self, AppErrorCode};

const APP_CONFIG_DIR: &str = "snappix";
const APP_CONFIG_FILE: &str = "app_config.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub version: String,
    pub description: String,
    pub hotkeys: AppHotkeysConfig,
    pub language: LanguageConfig,
    pub theme: ThemeConfig,
    pub pressed_keys: PressedKeysConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppHotkeysConfig {
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageConfig {
    pub current: String,
    pub fallback: String,
    pub supported: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeConfig {
    pub current: String,
    pub builtin: Vec<String>,
    pub custom: ThemePaletteConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ThemePaletteConfig {
    pub primary_bg: String,
    pub secondary_bg: String,
    pub primary_text: String,
    pub secondary_text: String,
    pub disabled_text: String,
    pub border_color: String,
    pub divider_color: String,
    pub hover_color: String,
    pub active_color: String,
    pub button_color: String,
    pub tooltip_bg: String,
    pub overlay_bg: String,
    pub overlay_strong_bg: String,
    pub shadow_color: String,
    pub shadow_size: f32,
    pub border_radius: f32,
    pub error_bg: String,
    pub error_text: String,
    pub success_color: String,
    pub warning_color: String,
    pub selection_border: String,
    pub selection_fill: String,
    pub selection_panel_bg: String,
    pub selection_text: String,
    pub on_accent_text: String,
    pub canvas_bg: String,
    pub canvas_grid_color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PressedKeysConfig {
    pub enabled: bool,
    pub position: String,
    pub show_mouse_buttons: bool,
}

impl AppConfig {
    fn appdata_path() -> PathBuf {
        let config_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(APP_CONFIG_DIR);
        config_dir.join(APP_CONFIG_FILE)
    }

    pub fn load_default() -> Self {
        let default_json = include_str!("../../config/app_config.json");
        serde_json::from_str(default_json).expect("Failed to parse default app config")
    }

    fn sanitize(&mut self) {
        if self.version.trim().is_empty() {
            self.version = "1.0.0".to_string();
        }
        if self.description.trim().is_empty() {
            self.description = "Global editor configuration for Snappix".to_string();
        }

        if self.hotkeys.source.trim().is_empty() {
            self.hotkeys.source = "appdata/hotkeys.bin".to_string();
        }

        let mut supported = self
            .language
            .supported
            .iter()
            .map(|lang| lang.trim().to_lowercase())
            .filter(|lang| lang == "ru" || lang == "en")
            .collect::<Vec<_>>();
        supported.sort();
        supported.dedup();
        if supported.is_empty() {
            supported = vec!["en".to_string(), "ru".to_string()];
        }
        self.language.supported = supported;

        self.language.current = self.language.current.trim().to_lowercase();
        self.language.fallback = self.language.fallback.trim().to_lowercase();
        if !self.language.supported.contains(&self.language.fallback) {
            self.language.fallback = "en".to_string();
        }
        if !self.language.supported.contains(&self.language.current) {
            self.language.current = self.language.fallback.clone();
        }

        if self.theme.builtin.is_empty() {
            self.theme.builtin = vec![
                "dark".to_string(),
                "light".to_string(),
                "custom".to_string(),
            ];
        }
        self.theme.current = self.theme.current.trim().to_lowercase();
        if !matches!(self.theme.current.as_str(), "dark" | "light" | "custom") {
            self.theme.current = "dark".to_string();
        }

        self.theme.custom.sanitize();

        self.pressed_keys.position = self.pressed_keys.position.trim().to_lowercase();
        if self.pressed_keys.position != "bottom-right" {
            self.pressed_keys.position = "bottom-right".to_string();
        }
    }

    pub fn load_or_initialize() -> Self {
        let path = Self::appdata_path();
        if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(content) => match serde_json::from_str::<AppConfig>(&content) {
                    Ok(mut config) => {
                        config.sanitize();
                        if let Err(err) = config.save() {
                            eprintln!(
                                "Failed to persist sanitized app config to {}: {err}",
                                path.display()
                            );
                            app_errors::report(AppErrorCode::AppConfigWriteFailed);
                        }
                        return config;
                    }
                    Err(err) => {
                        eprintln!(
                            "Failed to parse app config at {}: {err}. Recreating defaults.",
                            path.display()
                        );
                        app_errors::report(AppErrorCode::AppConfigParseFailed);
                    }
                },
                Err(err) => {
                    eprintln!(
                        "Failed to read app config at {}: {err}. Recreating defaults.",
                        path.display()
                    );
                    app_errors::report(AppErrorCode::AppConfigReadFailed);
                }
            }
        }

        let mut defaults = Self::load_default();
        defaults.sanitize();
        if let Err(err) = defaults.save() {
            eprintln!(
                "Failed to write default app config to {}: {err}",
                path.display()
            );
            app_errors::report(AppErrorCode::AppConfigWriteFailed);
        }
        defaults
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = Self::appdata_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
}

impl ThemePaletteConfig {
    fn sanitize(&mut self) {
        self.primary_bg = normalize_hex(&self.primary_bg, "#20242B");
        self.secondary_bg = normalize_hex(&self.secondary_bg, "#2B313A");
        self.primary_text = normalize_hex(&self.primary_text, "#F5F7FA");
        self.secondary_text = normalize_hex(&self.secondary_text, "#98A4B3");
        self.disabled_text = normalize_hex(&self.disabled_text, "#677382");
        self.border_color = normalize_hex(&self.border_color, "#3C4552");
        self.divider_color = normalize_hex(&self.divider_color, "#444D5A");
        self.hover_color = normalize_hex(&self.hover_color, "#323B47");
        self.active_color = normalize_hex(&self.active_color, "#FFD24D");
        self.button_color = normalize_hex(&self.button_color, "#445066");
        self.tooltip_bg = normalize_hex(&self.tooltip_bg, "#1F2630");
        self.overlay_bg = normalize_hex(&self.overlay_bg, "#00000066");
        self.overlay_strong_bg = normalize_hex(&self.overlay_strong_bg, "#00000090");
        self.shadow_color = normalize_hex(&self.shadow_color, "#00000040");
        self.shadow_size = normalize_metric(self.shadow_size, 10.0, 0.0, 80.0);
        self.border_radius = normalize_metric(self.border_radius, 6.0, 0.0, 48.0);
        self.error_bg = normalize_hex(&self.error_bg, "#FF444420");
        self.error_text = normalize_hex(&self.error_text, "#FF6666");
        self.success_color = normalize_hex(&self.success_color, "#7DD97D");
        self.warning_color = normalize_hex(&self.warning_color, "#F5DC78");
        self.selection_border = normalize_hex(&self.selection_border, "#4EA1FF");
        self.selection_fill = normalize_hex(&self.selection_fill, "#4EA1FF33");
        self.selection_panel_bg = normalize_hex(&self.selection_panel_bg, "#10203ACC");
        self.selection_text = normalize_hex(&self.selection_text, "#D9E7FF");
        self.on_accent_text = normalize_hex(&self.on_accent_text, "#000000");
        self.canvas_bg = normalize_hex(&self.canvas_bg, "#0F0F0F");
        self.canvas_grid_color = normalize_hex(&self.canvas_grid_color, "#2B2B2B");
    }
}

impl Default for ThemePaletteConfig {
    fn default() -> Self {
        Self {
            primary_bg: "#20242B".to_string(),
            secondary_bg: "#2B313A".to_string(),
            primary_text: "#F5F7FA".to_string(),
            secondary_text: "#98A4B3".to_string(),
            disabled_text: "#677382".to_string(),
            border_color: "#3C4552".to_string(),
            divider_color: "#444D5A".to_string(),
            hover_color: "#323B47".to_string(),
            active_color: "#FFD24D".to_string(),
            button_color: "#445066".to_string(),
            tooltip_bg: "#1F2630".to_string(),
            overlay_bg: "#00000066".to_string(),
            overlay_strong_bg: "#00000090".to_string(),
            shadow_color: "#00000040".to_string(),
            shadow_size: 10.0,
            border_radius: 6.0,
            error_bg: "#FF444420".to_string(),
            error_text: "#FF6666".to_string(),
            success_color: "#7DD97D".to_string(),
            warning_color: "#F5DC78".to_string(),
            selection_border: "#4EA1FF".to_string(),
            selection_fill: "#4EA1FF33".to_string(),
            selection_panel_bg: "#10203ACC".to_string(),
            selection_text: "#D9E7FF".to_string(),
            on_accent_text: "#000000".to_string(),
            canvas_bg: "#0F0F0F".to_string(),
            canvas_grid_color: "#2B2B2B".to_string(),
        }
    }
}

fn normalize_hex(value: &str, fallback: &str) -> String {
    let hex = value.trim().trim_start_matches('#');
    let is_valid =
        (hex.len() == 6 || hex.len() == 8) && hex.chars().all(|ch| ch.is_ascii_hexdigit());
    if is_valid {
        format!("#{}", hex.to_uppercase())
    } else {
        fallback.to_string()
    }
}

fn normalize_metric(value: f32, fallback: f32, min: f32, max: f32) -> f32 {
    if value.is_finite() {
        value.clamp(min, max)
    } else {
        fallback
    }
}
