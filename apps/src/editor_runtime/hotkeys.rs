use std::collections::HashMap;

use slint::SharedString;

use crate::app_errors::{self, AppErrorCode};
use crate::{config, AppWindow, HotkeyBinding};

const HOTKEYS_VERSION: &str = "1.4.0";
const HOTKEYS_DESCRIPTION: &str = "Core hotkeys configuration for Snappix editor";

const REQUIRED_HOTKEYS: [(&str, &str, &str, &str); 23] = [
    (
        "editor",
        "select_all",
        "Ctrl+A",
        "Select all objects on canvas",
    ),
    ("editor", "undo", "Ctrl+Z", "Undo last action"),
    ("editor", "cut", "Ctrl+X", "Cut selected objects"),
    ("editor", "copy", "Ctrl+C", "Copy selected objects"),
    ("editor", "paste", "Ctrl+V", "Paste copied objects"),
    (
        "editor",
        "paste_replace",
        "Ctrl+R",
        "Paste clipboard and replace selected objects",
    ),
    ("editor", "redo", "Shift+Z", "Redo previously undone action"),
    (
        "project",
        "exit_to_project_select",
        "Ctrl+Q",
        "Exit project to project chooser",
    ),
    (
        "canvas",
        "open_create_menu",
        "W",
        "Open create menu at cursor",
    ),
    (
        "canvas",
        "create_comment",
        "Ctrl+M",
        "Create comment outside the active canvas",
    ),
    (
        "outline",
        "select_prev",
        "A",
        "Select previous object in outline",
    ),
    (
        "outline",
        "select_next",
        "D",
        "Select next object in outline",
    ),
    ("element", "group", "Ctrl+G", "Group selected objects"),
    ("element", "ungroup", "Shift+G", "Ungroup selected objects"),
    (
        "element",
        "flip_vertical",
        "Shift+V",
        "Flip selected objects vertically",
    ),
    (
        "element",
        "flip_horizontal",
        "Shift+H",
        "Flip selected objects horizontally",
    ),
    ("view", "open_explorer", "1", "Open explorer panel"),
    ("view", "open_library", "2", "Open library panel"),
    ("view", "toggle_ui", "Ctrl+/", "Show or hide editor UI"),
    (
        "view",
        "toggle_comments",
        "Shift+C",
        "Show or hide comments",
    ),
    (
        "view",
        "reset_view",
        "Ctrl+0",
        "Reset zoom to 100% and center the active canvas",
    ),
    ("editor", "delete", "Delete", "Delete selected objects"),
    (
        "editor",
        "rename_selected",
        "Alt+F2",
        "Rename selected object",
    ),
];

fn resolve_hotkey_combo(
    config: &config::HotkeysConfig,
    category: &str,
    action: &str,
    fallback: &str,
) -> config::hotkeys::KeyCombo {
    config
        .get_key(category, action)
        .map(config::hotkeys::parse_key_combo)
        .unwrap_or_else(|| config::hotkeys::parse_key_combo(fallback))
}

fn to_binding(combo: config::hotkeys::KeyCombo) -> HotkeyBinding {
    HotkeyBinding {
        key: SharedString::from(combo.key.to_lowercase()),
        ctrl: combo.ctrl,
        shift: combo.shift,
        alt: combo.alt,
    }
}

fn sanitize_hotkeys(current: &config::HotkeysConfig) -> config::HotkeysConfig {
    let mut hotkeys: HashMap<String, HashMap<String, config::hotkeys::HotkeyAction>> =
        HashMap::new();

    for (category, action, default_key, default_description) in REQUIRED_HOTKEYS {
        let key = current
            .get(category, action)
            .map(|value| value.key.trim())
            .filter(|value| !value.is_empty())
            .unwrap_or(default_key)
            .to_string();
        let description = current
            .get(category, action)
            .map(|value| value.description.trim())
            .filter(|value| !value.is_empty())
            .unwrap_or(default_description)
            .to_string();
        hotkeys.entry(category.to_string()).or_default().insert(
            action.to_string(),
            config::hotkeys::HotkeyAction { key, description },
        );
    }

    config::HotkeysConfig {
        version: HOTKEYS_VERSION.to_string(),
        description: HOTKEYS_DESCRIPTION.to_string(),
        hotkeys,
    }
}

pub fn configure_hotkeys(ui: &AppWindow) {
    let loaded = config::HotkeysConfig::load_or_initialize();
    let hotkeys = sanitize_hotkeys(&loaded);
    if let Err(err) = hotkeys.save_to_appdata() {
        eprintln!("Failed to save sanitized hotkeys config in AppData: {err}");
        app_errors::report(AppErrorCode::HotkeysSyncFailed);
    }

    ui.set_select_all_hotkey(to_binding(resolve_hotkey_combo(
        &hotkeys,
        "editor",
        "select_all",
        "Ctrl+A",
    )));
    ui.set_undo_hotkey(to_binding(resolve_hotkey_combo(
        &hotkeys, "editor", "undo", "Ctrl+Z",
    )));
    ui.set_redo_hotkey(to_binding(resolve_hotkey_combo(
        &hotkeys, "editor", "redo", "Shift+Z",
    )));
    ui.set_cut_hotkey(to_binding(resolve_hotkey_combo(
        &hotkeys, "editor", "cut", "Ctrl+X",
    )));
    ui.set_copy_hotkey(to_binding(resolve_hotkey_combo(
        &hotkeys, "editor", "copy", "Ctrl+C",
    )));
    ui.set_paste_hotkey(to_binding(resolve_hotkey_combo(
        &hotkeys, "editor", "paste", "Ctrl+V",
    )));
    ui.set_paste_replace_hotkey(to_binding(resolve_hotkey_combo(
        &hotkeys,
        "editor",
        "paste_replace",
        "Ctrl+R",
    )));
    ui.set_delete_hotkey(to_binding(resolve_hotkey_combo(
        &hotkeys, "editor", "delete", "Delete",
    )));
    ui.set_rename_hotkey(to_binding(resolve_hotkey_combo(
        &hotkeys,
        "editor",
        "rename_selected",
        "Alt+F2",
    )));
    ui.set_exit_project_hotkey(to_binding(resolve_hotkey_combo(
        &hotkeys,
        "project",
        "exit_to_project_select",
        "Ctrl+Q",
    )));
    ui.set_create_menu_hotkey(to_binding(resolve_hotkey_combo(
        &hotkeys,
        "canvas",
        "open_create_menu",
        "W",
    )));
    ui.set_create_comment_hotkey(to_binding(resolve_hotkey_combo(
        &hotkeys,
        "canvas",
        "create_comment",
        "Ctrl+M",
    )));
    ui.set_outline_prev_hotkey(to_binding(resolve_hotkey_combo(
        &hotkeys,
        "outline",
        "select_prev",
        "A",
    )));
    ui.set_outline_next_hotkey(to_binding(resolve_hotkey_combo(
        &hotkeys,
        "outline",
        "select_next",
        "D",
    )));
    ui.set_group_hotkey(to_binding(resolve_hotkey_combo(
        &hotkeys, "element", "group", "Ctrl+G",
    )));
    ui.set_ungroup_hotkey(to_binding(resolve_hotkey_combo(
        &hotkeys, "element", "ungroup", "Shift+G",
    )));
    ui.set_flip_vertical_hotkey(to_binding(resolve_hotkey_combo(
        &hotkeys,
        "element",
        "flip_vertical",
        "Shift+V",
    )));
    ui.set_flip_horizontal_hotkey(to_binding(resolve_hotkey_combo(
        &hotkeys,
        "element",
        "flip_horizontal",
        "Shift+H",
    )));
    ui.set_explorer_hotkey(to_binding(resolve_hotkey_combo(
        &hotkeys,
        "view",
        "open_explorer",
        "1",
    )));
    ui.set_library_hotkey(to_binding(resolve_hotkey_combo(
        &hotkeys,
        "view",
        "open_library",
        "2",
    )));
    ui.set_toggle_ui_hotkey(to_binding(resolve_hotkey_combo(
        &hotkeys,
        "view",
        "toggle_ui",
        "Ctrl+/",
    )));
    ui.set_toggle_comments_hotkey(to_binding(resolve_hotkey_combo(
        &hotkeys,
        "view",
        "toggle_comments",
        "Shift+C",
    )));
    ui.set_reset_view_hotkey(to_binding(resolve_hotkey_combo(
        &hotkeys,
        "view",
        "reset_view",
        "Ctrl+0",
    )));
}
