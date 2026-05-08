//! Application scene navigation.

use shared::{log_fields, LogCategory, LogLevel, LogMessage};
use slint::ComponentHandle;

use crate::{AppScene, AppWindow};

pub fn setup_navigation_handler(ui: &AppWindow) {
    let ui_weak = ui.as_weak();

    ui.on_authenticated(move || {
        if let Some(ui) = ui_weak.upgrade() {
            log_fields(
                LogLevel::Info,
                LogCategory::App,
                LogMessage::AppSceneChanged,
                [("scene", "ProjectSelected")],
            );
            ui.set_current_scene(AppScene::ProjectSelected);
        }
    });
}
