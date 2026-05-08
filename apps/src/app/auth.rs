//! Authentication logic with session persistence.

use shared::{log, log_fields, LogCategory, LogLevel, LogMessage};
use slint::ComponentHandle;
use std::thread;

use crate::config::user::UserSession;
use crate::AppWindow;

pub fn try_auto_login(ui: &AppWindow) -> bool {
    if let Some(session) = UserSession::load() {
        if session.is_authenticated {
            ui.set_is_loading(false);
            log_fields(
                LogLevel::Info,
                LogCategory::App,
                LogMessage::UserLoginSucceeded,
                [("username", session.username.as_str())],
            );
            return true;
        }
    }
    false
}

/// Setup login handler with session persistence.
pub fn setup_login_handler(ui: &AppWindow) {
    let ui_weak = ui.as_weak();

    ui.on_login(
        move |email_or_username: slint::SharedString, password: slint::SharedString| {
            let ui_weak_clone = ui_weak.clone();

            if let Some(ui) = ui_weak.upgrade() {
                ui.set_is_loading(true);
                ui.set_error_message(slint::SharedString::from(""));
            }

            let username = email_or_username.to_string();
            let _pass = password.to_string();

            thread::spawn(move || {
                // Simulate authentication delay
                std::thread::sleep(std::time::Duration::from_millis(500));

                // Create authenticated session (stub - always succeeds)
                let session =
                    UserSession::authenticated(&username, &format!("{}@example.com", username));
                session.save().ok();

                slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_weak_clone.upgrade() {
                        ui.set_is_loading(false);
                        ui.set_error_message(slint::SharedString::from(""));
                        log_fields(
                            LogLevel::Info,
                            LogCategory::App,
                            LogMessage::UserLoginSucceeded,
                            [("username", username.as_str())],
                        );
                        ui.invoke_authenticated();
                    }
                })
                .ok();
            });
        },
    );
}

/// Setup logout handler.
pub fn setup_logout_handler(ui: &AppWindow) {
    let ui_weak = ui.as_weak();

    ui.on_logout(move || {
        let mut session = UserSession::default();
        session.logout();

        if let Some(ui) = ui_weak.upgrade() {
            log(LogLevel::Info, LogCategory::App, LogMessage::UserLogout);
            // Navigate back to auth scene
            ui.set_current_scene(crate::AppScene::Auth);
        }
    });
}
