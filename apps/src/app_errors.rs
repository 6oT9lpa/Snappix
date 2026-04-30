use slint::{ComponentHandle, SharedString, VecModel};
use std::cell::RefCell;
use std::rc::Rc;

use crate::{AppErrorNotification, AppWindow};

const MAX_ERROR_NOTIFICATIONS: usize = 8;

thread_local! {
    static ERROR_QUEUE: RefCell<Vec<AppErrorNotification>> = const { RefCell::new(Vec::new()) };
    static NEXT_ERROR_ID: RefCell<u64> = const { RefCell::new(0) };
    static ACTIVE_UI: RefCell<Option<slint::Weak<AppWindow>>> = const { RefCell::new(None) };
}

#[derive(Clone, Copy, Debug)]
pub enum AppErrorCode {
    ProjectLoadFailed,
    ProjectSaveFailed,
    ProjectDeleteFailed,
    ProjectRenameFailed,
    ProjectRelocateFailed,
    AppConfigReadFailed,
    AppConfigParseFailed,
    AppConfigWriteFailed,
    HotkeysLoadFailed,
    HotkeysWriteFailed,
    HotkeysSyncFailed,
}

impl AppErrorCode {
    fn message_key(self) -> &'static str {
        match self {
            Self::ProjectLoadFailed => {
                "Could not open the project. Check the file path and access rights."
            }
            Self::ProjectSaveFailed => {
                "Could not save project changes. Please check disk access and free space."
            }
            Self::ProjectDeleteFailed => {
                "Could not delete project files. Check file access and try again."
            }
            Self::ProjectRenameFailed => "Could not rename project. Please check the new name.",
            Self::ProjectRelocateFailed => {
                "Could not change project location. Check file access and selected folder."
            }
            Self::AppConfigReadFailed => {
                "Application settings could not be read. Default settings were restored."
            }
            Self::AppConfigParseFailed => {
                "Application settings are corrupted. Default settings were restored."
            }
            Self::AppConfigWriteFailed => {
                "Could not save application settings. Some preferences may not persist."
            }
            Self::HotkeysLoadFailed => {
                "Hotkey settings could not be read. Default hotkeys were restored."
            }
            Self::HotkeysWriteFailed => "Could not save hotkey settings. Changes may be temporary.",
            Self::HotkeysSyncFailed => "Could not update hotkey configuration. Please try again.",
        }
    }
}

pub fn attach_ui(ui: &AppWindow) {
    ACTIVE_UI.with(|slot| {
        *slot.borrow_mut() = Some(ui.as_weak());
    });
    sync_to_ui(ui);
}

pub fn report(code: AppErrorCode) {
    report_with_details(code, "");
}

pub fn report_with_details(code: AppErrorCode, details: impl Into<String>) {
    push_error(code.message_key(), details.into());
    sync_to_active_ui();
}

pub fn dismiss(error_id: &str) {
    ERROR_QUEUE.with(|queue| {
        queue
            .borrow_mut()
            .retain(|error| error.id.as_str() != error_id);
    });
    sync_to_active_ui();
}

fn push_error(message_key: &str, details: String) {
    let id = next_error_id();
    let trimmed_details = details.trim().to_string();

    ERROR_QUEUE.with(|queue| {
        let mut queue = queue.borrow_mut();
        queue.push(AppErrorNotification {
            id: SharedString::from(id),
            message: SharedString::from(message_key),
            details: SharedString::from(trimmed_details),
        });

        if queue.len() > MAX_ERROR_NOTIFICATIONS {
            let overflow = queue.len() - MAX_ERROR_NOTIFICATIONS;
            queue.drain(0..overflow);
        }
    });
}

fn next_error_id() -> String {
    NEXT_ERROR_ID.with(|counter| {
        let mut value = counter.borrow_mut();
        *value += 1;
        format!("app-error-{}", *value)
    })
}

fn sync_to_active_ui() {
    ACTIVE_UI.with(|slot| {
        let Some(ui_weak) = slot.borrow().as_ref().cloned() else {
            return;
        };

        let Some(ui) = ui_weak.upgrade() else {
            return;
        };

        sync_to_ui(&ui);
    });
}

fn sync_to_ui(ui: &AppWindow) {
    let notifications = ERROR_QUEUE.with(|queue| queue.borrow().clone());
    ui.set_app_errors(Rc::new(VecModel::from(notifications)).into());
}
