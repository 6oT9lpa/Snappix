//! Навигация между сценами приложения

use slint::ComponentHandle;

use crate::{AppScene, AppWindow};

/// Настройка обработчика навигации
pub fn setup_navigation_handler(ui: &AppWindow) {
    let ui_weak = ui.as_weak();

    ui.on_authenticated(move || {
        if let Some(ui) = ui_weak.upgrade() {
            println!("Переход на сцену ProjectSelected");
            ui.set_current_scene(AppScene::ProjectSelected);
        }
    });
}
