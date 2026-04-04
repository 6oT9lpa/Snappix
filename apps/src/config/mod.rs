//! Configuration module for hotkeys and settings.

pub mod app_config;
pub mod hotkeys;
pub mod recent_projects;
pub mod user;

pub use app_config::AppConfig;
pub use hotkeys::HotkeysConfig;
pub use recent_projects::RecentProjectsStorage;
