use arboard::Clipboard;
use image::{ImageFormat, RgbaImage};
use rfd::FileDialog;
use slint::{ComponentHandle, Model, SharedString};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::app;
use crate::app::project::{DevMode, PageSize, Platform, Project};
use crate::app_errors::{self, AppErrorCode};
use crate::config;
use crate::editor_runtime::{
    helpers,
    history::{HistoryActionKind, PreviewSelection, ProjectSnapshot},
    state::{EditorState, ProjectManagerHandleExt, TransformPreviewState},
    sync,
};
use crate::{AppScene, AppWindow, StringSearch};
use project_manager::operations;

const PREVIEW_SYNC_INTERVAL: std::time::Duration = std::time::Duration::from_millis(16);

pub fn register_callbacks(ui: &AppWindow, state: &EditorState) {
    register_window_callbacks(ui, state);
    register_notification_callbacks(ui);
    register_project_callbacks(ui, state);
    register_page_callbacks(ui, state);
    register_element_callbacks(ui, state);
    register_comment_callbacks(ui, state);
    register_hotkey_callbacks(ui, state);
    register_search_helpers(ui);
}

fn register_window_callbacks(ui: &AppWindow, state: &EditorState) {
    let ui_weak = ui.as_weak();
    let close_window_state = state.clone();
    ui.on_close_window(move || {
        if ui_weak.upgrade().is_some() {
            let mut pm = close_window_state.project_manager.borrow_mut();
            if let Some(project) = pm.current_project_mut() {
                save_project_history_silent(project, &close_window_state);
            }
            drop(pm);

            if let Err(err) = slint::quit_event_loop() {
                eprintln!("Failed to quit event loop on app close: {err}");
            }
        }
    });

    let ui_weak = ui.as_weak();
    ui.on_minimized_window(move |enable| {
        if let Some(ui) = ui_weak.upgrade() {
            ui.window().set_minimized(enable);
        }
    });

    let ui_weak = ui.as_weak();
    ui.on_maximized_window(move |enable| {
        if let Some(ui) = ui_weak.upgrade() {
            ui.set_window_maximized(enable);
            ui.window().set_maximized(enable);
            apply_native_window_rounding(&ui);
        }
    });

    let ui_weak = ui.as_weak();
    ui.on_start_window_drag(move || {
        let Some(ui) = ui_weak.upgrade() else {
            return;
        };
        request_native_window_drag(&ui);
    });

    let ui_weak = ui.as_weak();
    ui.on_start_window_resize(move |direction| {
        let Some(ui) = ui_weak.upgrade() else {
            return;
        };
        request_native_window_resize(&ui, direction.as_str());
    });

    let ui_weak = ui.as_weak();
    slint::Timer::single_shot(std::time::Duration::from_millis(0), move || {
        if let Some(ui) = ui_weak.upgrade() {
            apply_native_window_rounding(&ui);
            sync_window_maximized_state(&ui);
        }
    });

    use slint::winit_030::{winit, EventResult, WinitWindowAccessor};
    let ui_weak = ui.as_weak();
    ui.window().on_winit_window_event(move |_window, event| {
        match event {
            winit::event::WindowEvent::Resized(_)
            | winit::event::WindowEvent::Moved(_)
            | winit::event::WindowEvent::ScaleFactorChanged { .. } => {
                if let Some(ui) = ui_weak.upgrade() {
                    sync_window_maximized_state(&ui);
                }
            }
            _ => {}
        }
        EventResult::Propagate
    });
}

#[cfg(target_os = "windows")]
fn window_hwnd(ui: &AppWindow) -> Option<winapi::shared::windef::HWND> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let window_handle = ui.window().window_handle();
    let handle = window_handle.window_handle().ok()?;
    match handle.as_raw() {
        RawWindowHandle::Win32(win32) => {
            Some(win32.hwnd.get() as *mut winapi::ctypes::c_void as winapi::shared::windef::HWND)
        }
        _ => None,
    }
}

#[cfg(not(target_os = "windows"))]
fn window_hwnd(_ui: &AppWindow) -> Option<*mut std::ffi::c_void> {
    None
}

#[cfg(target_os = "windows")]
fn request_native_window_resize(ui: &AppWindow, direction: &str) {
    use winapi::um::winuser::{
        ReleaseCapture, SendMessageW, HTBOTTOM, HTBOTTOMLEFT, HTBOTTOMRIGHT, HTLEFT, HTRIGHT,
        HTTOP, HTTOPLEFT, HTTOPRIGHT, WM_NCLBUTTONDOWN,
    };

    if ui.window().is_maximized() {
        return;
    }

    let hit_test = match direction {
        "left" => HTLEFT,
        "right" => HTRIGHT,
        "top" => HTTOP,
        "bottom" => HTBOTTOM,
        "top-left" => HTTOPLEFT,
        "top-right" => HTTOPRIGHT,
        "bottom-left" => HTBOTTOMLEFT,
        "bottom-right" => HTBOTTOMRIGHT,
        _ => return,
    };

    let Some(hwnd) = window_hwnd(ui) else {
        return;
    };

    unsafe {
        ReleaseCapture();
        SendMessageW(hwnd, WM_NCLBUTTONDOWN, hit_test as usize, 0);
    }
}

#[cfg(not(target_os = "windows"))]
fn request_native_window_resize(_ui: &AppWindow, _direction: &str) {}

fn request_native_window_drag(ui: &AppWindow) {
    use slint::winit_030::WinitWindowAccessor;

    let started = ui
        .window()
        .with_winit_window(|window| window.drag_window().is_ok())
        .unwrap_or(false);
    if started {
        return;
    }

    #[cfg(target_os = "windows")]
    {
        use winapi::um::winuser::{ReleaseCapture, SendMessageW, HTCAPTION, WM_NCLBUTTONDOWN};

        let Some(hwnd) = window_hwnd(ui) else {
            return;
        };
        unsafe {
            ReleaseCapture();
            SendMessageW(hwnd, WM_NCLBUTTONDOWN, HTCAPTION as usize, 0);
        }
    }
}

fn sync_window_maximized_state(ui: &AppWindow) {
    use slint::winit_030::WinitWindowAccessor;

    let is_maximized = ui
        .window()
        .with_winit_window(|window| window.is_maximized())
        .unwrap_or(ui.get_window_maximized());

    if ui.get_window_maximized() != is_maximized {
        ui.set_window_maximized(is_maximized);
    }

    apply_native_window_rounding(ui);
}

#[cfg(target_os = "windows")]
fn apply_native_window_rounding(ui: &AppWindow) {
    use winapi::um::dwmapi::DwmSetWindowAttribute;

    // Windows 11 rounded corner hint.
    const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
    const DWMWCP_DEFAULT: u32 = 0;
    const DWMWCP_ROUND: u32 = 2;

    let Some(hwnd) = window_hwnd(ui) else {
        return;
    };

    // Avoid SetWindowRgn because it conflicts with snap/auto placement and may clip toolbar.
    let preference: u32 = if ui.window().is_maximized() {
        DWMWCP_DEFAULT
    } else {
        DWMWCP_ROUND
    };
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &preference as *const _ as *const winapi::ctypes::c_void,
            std::mem::size_of::<u32>() as u32,
        );
    }
}

#[cfg(not(target_os = "windows"))]
fn apply_native_window_rounding(_ui: &AppWindow) {}

fn register_notification_callbacks(ui: &AppWindow) {
    ui.on_dismiss_app_error_internal(move |error_id| {
        app_errors::dismiss(error_id.as_str());
    });
}

fn register_project_callbacks(ui: &AppWindow, state: &EditorState) {
    let ui_weak = ui.as_weak();
    let create_project_state = state.clone();
    ui.on_create_project_internal(
        move |name,
              path,
              platform_idx,
              dev_mode_idx,
              page_size_idx,
              custom_width,
              custom_height| {
            if name.is_empty() {
                if let Some(ui) = ui_weak.upgrade() {
                    ui.set_project_error_message(SharedString::from(
                        "Project name cannot be empty",
                    ));
                }
                return;
            }

            let sanitized_name = sanitize_name(name.as_str());
            if sanitized_name != name.as_str() {
                if let Some(ui) = ui_weak.upgrade() {
                    ui.set_project_error_message(SharedString::from(
                        "Project name contains invalid characters",
                    ));
                }
                return;
            }

            if path.is_empty() {
                if let Some(ui) = ui_weak.upgrade() {
                    ui.set_project_error_message(SharedString::from(
                        "Please select a project location",
                    ));
                }
                return;
            }

            let spx_path =
                std::path::Path::new(path.as_str()).join(format!("{}.spx", sanitized_name));
            if spx_path.exists() {
                if let Some(ui) = ui_weak.upgrade() {
                    ui.set_project_error_message(SharedString::from(
                        "A project with this name already exists in the selected location",
                    ));
                }
                return;
            }

            let mut pm = create_project_state.project_manager.borrow_mut();
            let project = pm.create_project(
                &sanitized_name,
                path.as_ref(),
                parse_platform(platform_idx),
                parse_dev_mode(dev_mode_idx),
                parse_page_size(page_size_idx),
                custom_width as u32,
                custom_height as u32,
            );

            helpers::save_project_silent(project);

            let mut recent_storage = config::RecentProjectsStorage::load();
            recent_storage.add_project(&sanitized_name, &project.spx_file_path().to_string_lossy());

            if let Some(ui) = ui_weak.upgrade() {
                ui.set_project_error_message(SharedString::from(""));
                ui.set_show_new_project_dialog(false);
                activate_project(&ui, project, &create_project_state);
                sync::set_recent_projects(&ui, &recent_storage);
            }
        },
    );

    let ui_weak = ui.as_weak();
    ui.on_browse_project_path(move || {
        let Some(ui) = ui_weak.upgrade() else {
            return;
        };

        if let Some(path) = FileDialog::new()
            .set_title("Select Project Location")
            .pick_folder()
        {
            ui.set_project_path(SharedString::from(path.to_string_lossy().to_string()));
        }
    });

    let ui_weak = ui.as_weak();
    let open_project_state = state.clone();
    ui.on_open_project_internal(move || {
        let Some(ui) = ui_weak.upgrade() else {
            return;
        };

        if let Some(path) = FileDialog::new()
            .set_title("Open Project")
            .add_filter("Snappix Project", &["spx"])
            .pick_file()
        {
            load_project_from_path(&ui, &open_project_state, &path.to_string_lossy());
        }
    });

    let ui_weak = ui.as_weak();
    let load_project_state = state.clone();
    ui.on_load_project_internal(move |path| {
        let Some(ui) = ui_weak.upgrade() else {
            return;
        };
        load_project_from_path(&ui, &load_project_state, path.as_str());
    });

    let ui_weak = ui.as_weak();
    ui.on_remove_recent_project_internal(move |path, name| {
        let Some(ui) = ui_weak.upgrade() else {
            return;
        };

        if let Err(err) = delete_project_from_disk(path.as_str(), name.as_str()) {
            eprintln!(
                "Failed to delete project from disk (path='{}', name='{}'): {err}",
                path, name
            );
            app_errors::report(AppErrorCode::ProjectDeleteFailed);
            return;
        }

        let mut recent_storage = config::RecentProjectsStorage::load();
        recent_storage.remove_project_entry(path.as_str(), name.as_str());
        sync::set_recent_projects(&ui, &recent_storage);
    });

    let ui_weak = ui.as_weak();
    ui.on_rename_recent_project_internal(move |path, name, new_name| {
        let Some(ui) = ui_weak.upgrade() else {
            return;
        };

        let trimmed = new_name.trim();
        if trimmed.is_empty() {
            app_errors::report(AppErrorCode::ProjectRenameFailed);
            return;
        }

        let mut recent_storage = config::RecentProjectsStorage::load();
        if !recent_storage.rename_project_entry(path.as_str(), name.as_str(), trimmed) {
            app_errors::report(AppErrorCode::ProjectRenameFailed);
            return;
        }
        sync::set_recent_projects(&ui, &recent_storage);
    });

    let ui_weak = ui.as_weak();
    ui.on_relocate_recent_project_internal(move |path, name| {
        let Some(ui) = ui_weak.upgrade() else {
            return;
        };

        let Some(new_path) = relocate_project_path(path.as_str(), name.as_str()) else {
            return;
        };

        let mut recent_storage = config::RecentProjectsStorage::load();
        if !recent_storage.relocate_project_entry(path.as_str(), name.as_str(), &new_path) {
            app_errors::report(AppErrorCode::ProjectRelocateFailed);
            return;
        }
        sync::set_recent_projects(&ui, &recent_storage);
    });
}

fn register_page_callbacks(ui: &AppWindow, state: &EditorState) {
    let ui_weak = ui.as_weak();
    let create_page_state = state.clone();
    ui.on_create_page_internal(move |name, width, height| {
        let Some(ui) = ui_weak.upgrade() else {
            return;
        };
        if !exit_history_preview_mode(&ui, &create_page_state) {
            return;
        }

        let mut pm = create_page_state.project_manager.borrow_mut();
        let Some(project) = pm.current_project_mut() else {
            return;
        };
        let before_snapshot = capture_project_snapshot(project, &create_page_state);

        let page_name = if name.trim().is_empty() {
            format!("Page {}", project.page_names().len() + 1)
        } else {
            name.to_string()
        };

        project.add_page(&page_name, width as u32, height as u32);
        helpers::save_project_silent(project);

        clear_selection_and_outline(&create_page_state);
        sync::sync_editor_models(&ui, project);
        apply_scene_for_active_document(&ui, project);
        if let (Some(before_snapshot), Some(after_snapshot)) = (
            before_snapshot,
            capture_project_snapshot(project, &create_page_state),
        ) {
            create_page_state.history.borrow_mut().record_change(
                HistoryActionKind::CreatePage,
                "Create page",
                format!("Added page: {}", page_name),
                before_snapshot,
                after_snapshot,
            );
        }
        refresh_canvas(&ui, project, &create_page_state);
    });

    let ui_weak = ui.as_weak();
    let close_page_state = state.clone();
    ui.on_close_document_internal(move |index| {
        let mut pm = close_page_state.project_manager.borrow_mut();
        let Some(project) = pm.current_project_mut() else {
            return;
        };

        let tab_index = index.max(0) as usize;
        if !project.close_document(tab_index) {
            return;
        }

        clear_selection_and_outline(&close_page_state);

        if let Some(ui) = ui_weak.upgrade() {
            sync::sync_editor_models(&ui, project);
            apply_scene_for_active_document(&ui, project);
            refresh_canvas(&ui, project, &close_page_state);
        }
    });

    let ui_weak = ui.as_weak();
    let select_page_state = state.clone();
    ui.on_select_document_internal(move |index| {
        let mut pm = select_page_state.project_manager.borrow_mut();
        let Some(project) = pm.current_project_mut() else {
            return;
        };

        let tab_index = index.max(0) as usize;
        if !project.select_open_document(tab_index) {
            return;
        }

        if let Some(ui) = ui_weak.upgrade() {
            clear_selection_and_outline(&select_page_state);
            sync::sync_editor_models(&ui, project);
            apply_scene_for_active_document(&ui, project);
            refresh_canvas(&ui, project, &select_page_state);
        }
    });

    let ui_weak = ui.as_weak();
    let open_project_page_state = state.clone();
    ui.on_open_project_page_internal(move |page_index| {
        let mut pm = open_project_page_state.project_manager.borrow_mut();
        let Some(project) = pm.current_project_mut() else {
            return;
        };

        let page_idx = page_index.max(0) as usize;
        if page_idx >= project.page_count() {
            return;
        }

        let Some(document) = project.page_document_ref(page_idx) else {
            return;
        };
        if !project.open_document(document) {
            return;
        }
        clear_selection_and_outline(&open_project_page_state);

        if let Some(ui) = ui_weak.upgrade() {
            sync::sync_editor_models(&ui, project);
            apply_scene_for_active_document(&ui, project);
            refresh_canvas(&ui, project, &open_project_page_state);
        }
    });

    let ui_weak = ui.as_weak();
    let open_page_blueprint_state = state.clone();
    ui.on_open_page_blueprint_internal(move |page_index| {
        let mut pm = open_page_blueprint_state.project_manager.borrow_mut();
        let Some(project) = pm.current_project_mut() else {
            return;
        };

        let page_idx = page_index.max(0) as usize;
        let Some(document) = project.page_blueprint_document_ref(page_idx) else {
            return;
        };
        if !project.open_document(document) {
            return;
        }

        if let Some(ui) = ui_weak.upgrade() {
            sync::sync_editor_models(&ui, project);
            apply_scene_for_active_document(&ui, project);
            refresh_canvas(&ui, project, &open_page_blueprint_state);
        }
    });

    let ui_weak = ui.as_weak();
    let open_server_blueprint_state = state.clone();
    ui.on_open_server_blueprint_internal(move || {
        let mut pm = open_server_blueprint_state.project_manager.borrow_mut();
        let Some(project) = pm.current_project_mut() else {
            return;
        };

        let Some(document) = project.server_blueprint_document_ref() else {
            return;
        };
        if !project.open_document(document) {
            return;
        }

        if let Some(ui) = ui_weak.upgrade() {
            sync::sync_editor_models(&ui, project);
            apply_scene_for_active_document(&ui, project);
            refresh_canvas(&ui, project, &open_server_blueprint_state);
        }
    });

    let ui_weak = ui.as_weak();
    let open_selected_element_blueprint_state = state.clone();
    ui.on_open_selected_element_blueprint_internal(move |element_id| {
        let mut pm = open_selected_element_blueprint_state
            .project_manager
            .borrow_mut();
        let Some(project) = pm.current_project_mut() else {
            return;
        };

        if let Some(element_id) = helpers::parse_uuid(element_id.as_str()) {
            let _ = project.ensure_element_event_nodes_on_active_page_blueprint(element_id);
            helpers::save_project_silent(project);
        }

        let Some(document) = project.page_blueprint_document_ref(project.active_page_index())
        else {
            return;
        };
        if !project.open_document(document) {
            return;
        }

        if let Some(ui) = ui_weak.upgrade() {
            sync::sync_editor_models(&ui, project);
            apply_scene_for_active_document(&ui, project);
            refresh_canvas(&ui, project, &open_selected_element_blueprint_state);
        }
    });

    let ui_weak = ui.as_weak();
    let add_blueprint_variable_state = state.clone();
    ui.on_add_blueprint_variable_internal(move || {
        let Some(ui) = ui_weak.upgrade() else {
            return;
        };
        if !exit_history_preview_mode(&ui, &add_blueprint_variable_state) {
            return;
        }

        let mut pm = add_blueprint_variable_state.project_manager.borrow_mut();
        let Some(project) = pm.current_project_mut() else {
            return;
        };
        let before_snapshot = capture_project_snapshot(project, &add_blueprint_variable_state);
        if project.add_local_variable_to_active_blueprint().is_none() {
            return;
        }
        helpers::save_project_silent(project);
        if let (Some(before_snapshot), Some(after_snapshot)) = (
            before_snapshot,
            capture_project_snapshot(project, &add_blueprint_variable_state),
        ) {
            add_blueprint_variable_state
                .history
                .borrow_mut()
                .record_change(
                    HistoryActionKind::ModifyObject,
                    "Add blueprint variable",
                    "Created bool variable",
                    before_snapshot,
                    after_snapshot,
                );
        }
        sync::sync_editor_models(&ui, project);
    });

    let ui_weak = ui.as_weak();
    let add_blueprint_variable_node_state = state.clone();
    ui.on_add_blueprint_variable_node_internal(move |variable_id, access_kind, x, y| {
        let Some(ui) = ui_weak.upgrade() else {
            return;
        };
        if !exit_history_preview_mode(&ui, &add_blueprint_variable_node_state) {
            return;
        }

        let Some(variable_id) = helpers::parse_uuid(variable_id.as_str()) else {
            return;
        };
        let mut pm = add_blueprint_variable_node_state
            .project_manager
            .borrow_mut();
        let Some(project) = pm.current_project_mut() else {
            return;
        };
        let before_snapshot = capture_project_snapshot(project, &add_blueprint_variable_node_state);
        if project
            .add_variable_node_to_active_blueprint(variable_id, access_kind.as_str(), x, y)
            .is_none()
        {
            return;
        }
        helpers::save_project_silent(project);
        if let (Some(before_snapshot), Some(after_snapshot)) = (
            before_snapshot,
            capture_project_snapshot(project, &add_blueprint_variable_node_state),
        ) {
            add_blueprint_variable_node_state
                .history
                .borrow_mut()
                .record_change(
                    HistoryActionKind::CreateObject,
                    "Add variable node",
                    format!("{} variable node", access_kind),
                    before_snapshot,
                    after_snapshot,
                );
        }
        sync::sync_editor_models(&ui, project);
        refresh_canvas(&ui, project, &add_blueprint_variable_node_state);
    });

    let ui_weak = ui.as_weak();
    let rename_blueprint_variable_state = state.clone();
    ui.on_rename_blueprint_variable_internal(move |variable_id, name| {
        let Some(ui) = ui_weak.upgrade() else {
            return;
        };
        if !exit_history_preview_mode(&ui, &rename_blueprint_variable_state) {
            return;
        }
        let Some(variable_id) = helpers::parse_uuid(variable_id.as_str()) else {
            return;
        };
        let mut pm = rename_blueprint_variable_state.project_manager.borrow_mut();
        let Some(project) = pm.current_project_mut() else {
            return;
        };
        let before_snapshot = capture_project_snapshot(project, &rename_blueprint_variable_state);
        if !project.rename_local_variable_in_active_blueprint(variable_id, name.as_str()) {
            return;
        }
        helpers::save_project_silent(project);
        if let (Some(before_snapshot), Some(after_snapshot)) = (
            before_snapshot,
            capture_project_snapshot(project, &rename_blueprint_variable_state),
        ) {
            rename_blueprint_variable_state
                .history
                .borrow_mut()
                .record_change(
                    HistoryActionKind::ModifyObject,
                    "Rename blueprint variable",
                    format!("Variable: {}", name),
                    before_snapshot,
                    after_snapshot,
                );
        }
        sync::sync_editor_models(&ui, project);
    });

    let ui_weak = ui.as_weak();
    let set_blueprint_variable_type_state = state.clone();
    ui.on_set_blueprint_variable_type_internal(move |variable_id, type_name| {
        let Some(ui) = ui_weak.upgrade() else {
            return;
        };
        if !exit_history_preview_mode(&ui, &set_blueprint_variable_type_state) {
            return;
        }
        let Some(variable_id) = helpers::parse_uuid(variable_id.as_str()) else {
            return;
        };
        let mut pm = set_blueprint_variable_type_state.project_manager.borrow_mut();
        let Some(project) = pm.current_project_mut() else {
            return;
        };
        let before_snapshot =
            capture_project_snapshot(project, &set_blueprint_variable_type_state);
        if !project.set_local_variable_type_in_active_blueprint(variable_id, type_name.as_str()) {
            return;
        }
        helpers::save_project_silent(project);
        if let (Some(before_snapshot), Some(after_snapshot)) = (
            before_snapshot,
            capture_project_snapshot(project, &set_blueprint_variable_type_state),
        ) {
            set_blueprint_variable_type_state
                .history
                .borrow_mut()
                .record_change(
                    HistoryActionKind::ModifyObject,
                    "Change blueprint variable type",
                    format!("Type: {}", type_name),
                    before_snapshot,
                    after_snapshot,
                );
        }
        sync::sync_editor_models(&ui, project);
    });

    let ui_weak = ui.as_weak();
    let add_blueprint_function_state = state.clone();
    ui.on_add_blueprint_function_internal(move || {
        let Some(ui) = ui_weak.upgrade() else {
            return;
        };
        if !exit_history_preview_mode(&ui, &add_blueprint_function_state) {
            return;
        }

        let mut pm = add_blueprint_function_state.project_manager.borrow_mut();
        let Some(project) = pm.current_project_mut() else {
            return;
        };
        let before_snapshot = capture_project_snapshot(project, &add_blueprint_function_state);
        if project.add_function_to_active_blueprint().is_none() {
            return;
        }
        helpers::save_project_silent(project);
        if let (Some(before_snapshot), Some(after_snapshot)) = (
            before_snapshot,
            capture_project_snapshot(project, &add_blueprint_function_state),
        ) {
            add_blueprint_function_state
                .history
                .borrow_mut()
                .record_change(
                    HistoryActionKind::CreateObject,
                    "Add blueprint function",
                    "Created user function",
                    before_snapshot,
                    after_snapshot,
                );
        }
        sync::sync_editor_models(&ui, project);
    });

    let ui_weak = ui.as_weak();
    let rename_project_page_state = state.clone();
    ui.on_rename_project_page_internal(move |page_index, new_name| {
        let Some(ui) = ui_weak.upgrade() else {
            return;
        };
        if !exit_history_preview_mode(&ui, &rename_project_page_state) {
            return;
        }

        let mut pm = rename_project_page_state.project_manager.borrow_mut();
        let Some(project) = pm.current_project_mut() else {
            return;
        };
        let before_snapshot = capture_project_snapshot(project, &rename_project_page_state);

        let page_idx = page_index.max(0) as usize;
        if !project.rename_page(page_idx, new_name.as_str()) {
            return;
        }

        helpers::save_project_silent(project);
        if let (Some(before_snapshot), Some(after_snapshot)) = (
            before_snapshot,
            capture_project_snapshot(project, &rename_project_page_state),
        ) {
            rename_project_page_state
                .history
                .borrow_mut()
                .record_change(
                    HistoryActionKind::RenamePage,
                    "Rename page",
                    format!("New page name: {}", new_name),
                    before_snapshot,
                    after_snapshot,
                );
        }

        // Avoid mutating Slint models in the same callback stack as TextInput.accepted
        // to prevent re-entrant UI model update panics during rename.
        let ui_for_sync = ui.as_weak();
        let state_for_sync = rename_project_page_state.clone();
        slint::Timer::single_shot(std::time::Duration::ZERO, move || {
            let Some(ui) = ui_for_sync.upgrade() else {
                return;
            };
            let mut pm = state_for_sync.project_manager.borrow_mut();
            let Some(project) = pm.current_project_mut() else {
                return;
            };
            sync::sync_editor_models(&ui, project);
            apply_scene_for_active_document(&ui, project);
            refresh_canvas(&ui, project, &state_for_sync);
        });
    });

    let ui_weak = ui.as_weak();
    let delete_project_page_state = state.clone();
    ui.on_delete_project_page_internal(move |page_index| {
        let Some(ui) = ui_weak.upgrade() else {
            return;
        };
        if !exit_history_preview_mode(&ui, &delete_project_page_state) {
            return;
        }

        let mut pm = delete_project_page_state.project_manager.borrow_mut();
        let Some(project) = pm.current_project_mut() else {
            return;
        };
        let before_snapshot = capture_project_snapshot(project, &delete_project_page_state);

        let page_idx = page_index.max(0) as usize;
        if !project.remove_page(page_idx) {
            return;
        }

        helpers::save_project_silent(project);
        clear_selection_and_outline(&delete_project_page_state);

        if let (Some(before_snapshot), Some(after_snapshot)) = (
            before_snapshot,
            capture_project_snapshot(project, &delete_project_page_state),
        ) {
            delete_project_page_state
                .history
                .borrow_mut()
                .record_change(
                    HistoryActionKind::DeletePage,
                    "Delete page",
                    "Page removed from project",
                    before_snapshot,
                    after_snapshot,
                );
        }

        // Defer UI model synchronization by one tick to avoid re-entrant model updates.
        let ui_for_sync = ui.as_weak();
        let state_for_sync = delete_project_page_state.clone();
        slint::Timer::single_shot(std::time::Duration::ZERO, move || {
            let Some(ui) = ui_for_sync.upgrade() else {
                return;
            };
            let mut pm = state_for_sync.project_manager.borrow_mut();
            let Some(project) = pm.current_project_mut() else {
                return;
            };
            sync::sync_editor_models(&ui, project);
            apply_scene_for_active_document(&ui, project);
            refresh_canvas(&ui, project, &state_for_sync);
        });
    });
}

fn register_element_callbacks(ui: &AppWindow, state: &EditorState) {
    let ui_weak = ui.as_weak();
    let add_element_state = state.clone();
    ui.on_add_element_internal(move |element_type, x, y, width, height, parent_id| {
        let Some(ui) = ui_weak.upgrade() else {
            return;
        };
        if !exit_history_preview_mode(&ui, &add_element_state) {
            return;
        }

        let mut pm = add_element_state.project_manager.borrow_mut();
        let Some(project) = pm.current_project_mut() else {
            return;
        };
        let before_snapshot = capture_project_snapshot(project, &add_element_state);

        let mut element = app::project::CanvasElementData::from_component_template(
            element_type.as_ref(),
            x,
            y,
        );
        if width > 0.0 {
            element.width = width;
        }
        if height > 0.0 {
            element.height = height;
        }

        helpers::clamp_element_to_page(project, &mut element);
        let resolved_parent = helpers::parse_uuid(parent_id.as_str())
            .or_else(|| helpers::find_drop_parent(project, x, y));
        let added_id = project.add_element_to_active_page_with_parent(element, resolved_parent);
        if added_id.is_none() {
            return;
        }
        helpers::save_project_silent(project);

        let mut selected = add_element_state.selected_elements.borrow_mut();
        selected.clear();
        if let Some(added_id) = added_id {
            selected.push(added_id);
        }
        drop(selected);

        if let (Some(before_snapshot), Some(after_snapshot)) = (
            before_snapshot,
            capture_project_snapshot(project, &add_element_state),
        ) {
            add_element_state.history.borrow_mut().record_change(
                HistoryActionKind::CreateObject,
                "Create object",
                format!("Added object type: {}", element_type),
                before_snapshot,
                after_snapshot,
            );
        }
        refresh_canvas(&ui, project, &add_element_state);
    });

    let ui_weak = ui.as_weak();
    let select_element_state = state.clone();
    ui.on_select_element_internal(move |id, additive, _deep_select| {
        let mut pm = select_element_state.project_manager.borrow_mut();
        let Some(project) = pm.current_project_mut() else {
            return;
        };

        let mut selected = select_element_state.selected_elements.borrow_mut();
        if id.is_empty() {
            selected.clear();
        } else if let Some(clicked_id) = helpers::parse_uuid(id.as_str()) {
            if additive {
                if let Some(pos) = selected.iter().position(|id| *id == clicked_id) {
                    selected.remove(pos);
                } else {
                    selected.push(clicked_id);
                }
            } else {
                selected.clear();
                selected.push(clicked_id);
            }
        } else {
            selected.clear();
        }

        if let Some(ui) = ui_weak.upgrade() {
            sync::sync_canvas(
                &ui,
                project,
                &selected,
                &select_element_state.collapsed_outline_nodes.borrow(),
            );
        }
    });

    let ui_weak = ui.as_weak();
    let marquee_select_state = state.clone();
    ui.on_marquee_select_internal(move |x1, y1, x2, y2, additive| {
        let mut pm = marquee_select_state.project_manager.borrow_mut();
        let Some(project) = pm.current_project_mut() else {
            return;
        };

        let select_min_x = x1.min(x2);
        let select_min_y = y1.min(y2);
        let select_max_x = x1.max(x2);
        let select_max_y = y1.max(y2);

        let mut selected = marquee_select_state.selected_elements.borrow_mut();
        let mut selected_set: HashSet<Uuid> = if additive {
            selected.iter().copied().collect()
        } else {
            selected.clear();
            HashSet::new()
        };

        for element in project.active_page_elements() {
            let (element_min_x, element_min_y, element_max_x, element_max_y) =
                rotated_selection_bounds(&element);
            if rects_intersect(
                select_min_x,
                select_min_y,
                select_max_x,
                select_max_y,
                element_min_x,
                element_min_y,
                element_max_x,
                element_max_y,
            ) && selected_set.insert(element.id)
            {
                selected.push(element.id);
            }
        }

        if let Some(ui) = ui_weak.upgrade() {
            sync::sync_canvas(
                &ui,
                project,
                &selected,
                &marquee_select_state.collapsed_outline_nodes.borrow(),
            );
        }
    });

    let ui_weak = ui.as_weak();
    let toggle_outline_state = state.clone();
    ui.on_toggle_outline_node_internal(move |id| {
        let mut pm = toggle_outline_state.project_manager.borrow_mut();
        let Some(project) = pm.current_project_mut() else {
            return;
        };

        if let Some(node_id) = helpers::parse_uuid(id.as_str()) {
            let mut collapsed = toggle_outline_state.collapsed_outline_nodes.borrow_mut();
            if collapsed.contains(&node_id) {
                collapsed.remove(&node_id);
            } else {
                collapsed.insert(node_id);
            }
        }

        if let Some(ui) = ui_weak.upgrade() {
            refresh_canvas(&ui, project, &toggle_outline_state);
        }
    });

    let ui_weak = ui.as_weak();
    let reparent_element_state = state.clone();
    ui.on_reparent_element_internal(move |child_id, parent_id| {
        let Some(ui) = ui_weak.upgrade() else {
            return;
        };
        if !exit_history_preview_mode(&ui, &reparent_element_state) {
            return;
        }

        let mut pm = reparent_element_state.project_manager.borrow_mut();
        let Some(project) = pm.current_project_mut() else {
            return;
        };
        let before_snapshot = capture_project_snapshot(project, &reparent_element_state);

        let Some(child_uuid) = helpers::parse_uuid(child_id.as_str()) else {
            return;
        };
        let parent_uuid = helpers::parse_uuid(parent_id.as_str());
        if !project.update_element_parent_on_active_page(child_uuid, parent_uuid) {
            return;
        }

        helpers::save_project_silent(project);
        if let (Some(before_snapshot), Some(after_snapshot)) = (
            before_snapshot,
            capture_project_snapshot(project, &reparent_element_state),
        ) {
            reparent_element_state.history.borrow_mut().record_change(
                HistoryActionKind::ReparentObject,
                "Reparent object",
                "Object moved in outline hierarchy",
                before_snapshot,
                after_snapshot,
            );
        }
        refresh_canvas(&ui, project, &reparent_element_state);
    });

    let ui_weak = ui.as_weak();
    let rename_element_state = state.clone();
    ui.on_rename_element_internal(move |id, new_name| {
        let Some(ui) = ui_weak.upgrade() else {
            return;
        };
        if !exit_history_preview_mode(&ui, &rename_element_state) {
            return;
        }

        let mut pm = rename_element_state.project_manager.borrow_mut();
        let Some(project) = pm.current_project_mut() else {
            return;
        };
        let before_snapshot = capture_project_snapshot(project, &rename_element_state);

        let Some(element_id) = helpers::parse_uuid(id.as_str()) else {
            return;
        };
        if !project.update_element_name_on_active_page(element_id, new_name.as_str()) {
            return;
        }

        helpers::save_project_silent(project);
        if let (Some(before_snapshot), Some(after_snapshot)) = (
            before_snapshot,
            capture_project_snapshot(project, &rename_element_state),
        ) {
            rename_element_state.history.borrow_mut().record_change(
                HistoryActionKind::RenameObject,
                "Rename object",
                format!("New name: {new_name}"),
                before_snapshot,
                after_snapshot,
            );
        }

        // Same protection as page rename: defer UI model synchronization by one tick.
        let ui_for_sync = ui.as_weak();
        let state_for_sync = rename_element_state.clone();
        slint::Timer::single_shot(std::time::Duration::ZERO, move || {
            let Some(ui) = ui_for_sync.upgrade() else {
                return;
            };
            let mut pm = state_for_sync.project_manager.borrow_mut();
            let Some(project) = pm.current_project_mut() else {
                return;
            };
            refresh_canvas(&ui, project, &state_for_sync);
        });
    });

    let ui_weak = ui.as_weak();
    let delete_element_state = state.clone();
    ui.on_delete_element_internal(move |id| {
        let Some(ui) = ui_weak.upgrade() else {
            return;
        };
        if !exit_history_preview_mode(&ui, &delete_element_state) {
            return;
        }

        let mut pm = delete_element_state.project_manager.borrow_mut();
        let Some(project) = pm.current_project_mut() else {
            return;
        };
        let before_snapshot = capture_project_snapshot(project, &delete_element_state);

        let delete_targets = if let Some(element_id) = helpers::parse_uuid(id.as_str()) {
            vec![element_id]
        } else {
            let selected_snapshot = delete_element_state.selected_elements.borrow().clone();
            selected_root_ids(project, &selected_snapshot)
        };
        if delete_targets.is_empty() {
            return;
        };

        let mut removed_count = 0usize;
        for element_id in delete_targets {
            if project.remove_element_on_active_page(element_id) {
                removed_count += 1;
            }
        }
        if removed_count == 0 {
            return;
        }

        helpers::save_project_silent(project);
        retain_existing_selection(project, &delete_element_state);

        if let (Some(before_snapshot), Some(after_snapshot)) = (
            before_snapshot,
            capture_project_snapshot(project, &delete_element_state),
        ) {
            delete_element_state.history.borrow_mut().record_change(
                HistoryActionKind::DeleteObject,
                "Delete object",
                format!("Objects removed from canvas: {removed_count}"),
                before_snapshot,
                after_snapshot,
            );
        }
        refresh_canvas(&ui, project, &delete_element_state);
    });

    let ui_weak = ui.as_weak();
    let group_selected_state = state.clone();
    ui.on_group_selected_internal(move || {
        let Some(ui) = ui_weak.upgrade() else {
            return;
        };
        if !exit_history_preview_mode(&ui, &group_selected_state) {
            return;
        }

        let mut pm = group_selected_state.project_manager.borrow_mut();
        let Some(project) = pm.current_project_mut() else {
            return;
        };
        let before_snapshot = capture_project_snapshot(project, &group_selected_state);

        let ordered = group_selected_state.selected_elements.borrow().clone();
        if ordered.len() < 2 {
            return;
        }

        let parent_id = ordered[0];
        if !project.group_elements_on_active_page(parent_id, &ordered[1..]) {
            return;
        }

        helpers::save_project_silent(project);
        select_only(&group_selected_state, parent_id);

        if let (Some(before_snapshot), Some(after_snapshot)) = (
            before_snapshot,
            capture_project_snapshot(project, &group_selected_state),
        ) {
            group_selected_state.history.borrow_mut().record_change(
                HistoryActionKind::Group,
                "Group objects",
                format!("Grouped objects: {}", ordered.len()),
                before_snapshot,
                after_snapshot,
            );
        }
        refresh_canvas(&ui, project, &group_selected_state);
    });

    let ui_weak = ui.as_weak();
    let ungroup_selected_state = state.clone();
    ui.on_ungroup_selected_internal(move || {
        let Some(ui) = ui_weak.upgrade() else {
            return;
        };
        if !exit_history_preview_mode(&ui, &ungroup_selected_state) {
            return;
        }

        let mut pm = ungroup_selected_state.project_manager.borrow_mut();
        let Some(project) = pm.current_project_mut() else {
            return;
        };
        let before_snapshot = capture_project_snapshot(project, &ungroup_selected_state);

        let selected_ids = ungroup_selected_state.selected_elements.borrow().clone();
        let mut changed = false;
        for id in selected_ids {
            changed |= project.ungroup_element_on_active_page(id);
        }
        if !changed {
            return;
        }

        helpers::save_project_silent(project);
        if let (Some(before_snapshot), Some(after_snapshot)) = (
            before_snapshot,
            capture_project_snapshot(project, &ungroup_selected_state),
        ) {
            ungroup_selected_state.history.borrow_mut().record_change(
                HistoryActionKind::Ungroup,
                "Ungroup objects",
                "Selected groups were ungrouped",
                before_snapshot,
                after_snapshot,
            );
        }
        refresh_canvas(&ui, project, &ungroup_selected_state);
    });

    let ui_weak = ui.as_weak();
    let update_geometry_state = state.clone();
    ui.on_update_element_geometry_internal(move |id, x, y, w, h, r| {
        let Some(ui) = ui_weak.upgrade() else {
            return;
        };
        if !is_finite_geometry(x, y, w, h, r) {
            return;
        }
        if !exit_history_preview_mode(&ui, &update_geometry_state) {
            return;
        }

        let mut pm = update_geometry_state.project_manager.borrow_mut();
        let Some(project) = pm.current_project_mut() else {
            return;
        };
        let before_snapshot = capture_project_snapshot(project, &update_geometry_state);

        let Some(element_id) = resolve_target_element_id(id.as_str(), &update_geometry_state)
        else {
            return;
        };

        let selected_snapshot = update_geometry_state.selected_elements.borrow().clone();
        if !apply_geometry_update(project, &selected_snapshot, element_id, x, y, w, h, r) {
            return;
        }

        helpers::save_project_silent(project);
        ensure_primary_selection(&update_geometry_state, element_id);

        if let (Some(before_snapshot), Some(after_snapshot)) = (
            before_snapshot,
            capture_project_snapshot(project, &update_geometry_state),
        ) {
            update_geometry_state.history.borrow_mut().record_change(
                HistoryActionKind::ModifyObject,
                "Modify object",
                "Object geometry changed",
                before_snapshot,
                after_snapshot,
            );
        }
        refresh_canvas(&ui, project, &update_geometry_state);
    });

    let ui_weak = ui.as_weak();
    let preview_sync_at_state =
        std::rc::Rc::new(std::cell::RefCell::new(None::<std::time::Instant>));
    let preview_geometry_state = state.clone();
    ui.on_preview_element_geometry_internal(move |id, x, y, w, h, r| {
        if !is_finite_geometry(x, y, w, h, r) {
            return;
        }
        let mut pm = preview_geometry_state.project_manager.borrow_mut();
        let Some(project) = pm.current_project_mut() else {
            return;
        };

        let Some(element_id) = helpers::parse_uuid(id.as_str()) else {
            return;
        };

        let selected_snapshot = preview_geometry_state.selected_elements.borrow().clone();
        let is_single_selected_preview =
            selected_snapshot.len() == 1 && selected_snapshot.first().copied() == Some(element_id);
        // For a single selected object, transform preview is fully local in Canvas
        // (move/resize/rotate). Commit is applied on pointer release.
        if is_single_selected_preview {
            return;
        }

        let is_move_preview = project
            .get_element_on_active_page(element_id)
            .map(|current| {
                (w - current.width).abs() <= 0.001
                    && (h - current.height).abs() <= 0.001
                    && (r - current.rotation).abs() <= 0.001
            })
            .unwrap_or(false);

        // Move preview is rendered locally in Canvas for both single and multi selection.
        // Skipping project mutations here removes frame spikes on large scenes.
        if is_move_preview {
            return;
        }

        capture_transform_preview(project, &preview_geometry_state, element_id);
        if !apply_geometry_update(project, &selected_snapshot, element_id, x, y, w, h, r) {
            return;
        }

        if let Some(ui) = ui_weak.upgrade() {
            let now = std::time::Instant::now();
            let should_sync = {
                let mut last_sync = preview_sync_at_state.borrow_mut();
                let allowed = last_sync
                    .map(|last_sync| now.duration_since(last_sync) >= PREVIEW_SYNC_INTERVAL)
                    .unwrap_or(true);
                if allowed {
                    *last_sync = Some(now);
                }
                allowed
            };
            if should_sync {
                refresh_canvas_preview(&ui, project, &preview_geometry_state);
            }
        }
    });

    let ui_weak = ui.as_weak();
    let finish_transform_state = state.clone();
    ui.on_finish_element_transform_internal(move |id, commit, x, y, w, h, r| {
        let Some(ui) = ui_weak.upgrade() else {
            return;
        };
        if !is_finite_geometry(x, y, w, h, r) {
            return;
        }

        let mut pm = finish_transform_state.project_manager.borrow_mut();
        let Some(project) = pm.current_project_mut() else {
            return;
        };
        let before_snapshot = commit
            .then(|| capture_project_snapshot(project, &finish_transform_state))
            .flatten();

        let Some(element_id) = helpers::parse_uuid(id.as_str()) else {
            return;
        };

        if !commit {
            let Some(preview) = take_transform_preview(&finish_transform_state, element_id) else {
                return;
            };
            let _ = project.restore_element_geometries_on_active_page(&preview.geometries);
            refresh_canvas_preview(&ui, project, &finish_transform_state);
            return;
        }

        let selected_snapshot = finish_transform_state.selected_elements.borrow().clone();
        let had_preview = has_transform_preview(&finish_transform_state, element_id);
        let changed = apply_geometry_update(project, &selected_snapshot, element_id, x, y, w, h, r);
        let _ = take_transform_preview(&finish_transform_state, element_id);
        if !changed && !had_preview {
            return;
        }

        helpers::save_project_silent(project);
        ensure_primary_selection(&finish_transform_state, element_id);

        if let (Some(before_snapshot), Some(after_snapshot)) = (
            before_snapshot,
            capture_project_snapshot(project, &finish_transform_state),
        ) {
            finish_transform_state.history.borrow_mut().record_change(
                HistoryActionKind::ModifyObject,
                "Transform object",
                "Object moved, resized or rotated",
                before_snapshot,
                after_snapshot,
            );
        }
        refresh_canvas(&ui, project, &finish_transform_state);
    });

    let ui_weak = ui.as_weak();
    let update_text_state = state.clone();
    ui.on_update_element_text_internal(move |id, text| {
        let Some(ui) = ui_weak.upgrade() else {
            return;
        };
        if !exit_history_preview_mode(&ui, &update_text_state) {
            return;
        }

        let mut pm = update_text_state.project_manager.borrow_mut();
        let Some(project) = pm.current_project_mut() else {
            return;
        };
        let before_snapshot = capture_project_snapshot(project, &update_text_state);

        let Some(element_id) = helpers::parse_uuid(id.as_str()) else {
            return;
        };
        if !project.update_element_text_on_active_page(element_id, &text) {
            return;
        }

        helpers::save_project_silent(project);

        if let (Some(before_snapshot), Some(after_snapshot)) = (
            before_snapshot,
            capture_project_snapshot(project, &update_text_state),
        ) {
            update_text_state.history.borrow_mut().record_change(
                HistoryActionKind::ModifyObject,
                "Edit object text",
                "Text content updated",
                before_snapshot,
                after_snapshot,
            );
        }
        refresh_canvas(&ui, project, &update_text_state);
    });

    let ui_weak = ui.as_weak();
    let update_style_state = state.clone();
    ui.on_update_element_style_internal(
        move |id,
              bg,
              border,
              bw,
              br,
              fs,
              tc,
              ff,
              tw,
              ph,
              bg_image,
              image_src,
              inherit_text_style,
              checked,
              checkbox_box_side,
              checkbox_check_color,
              checkbox_box_color,
              checkbox_box_border_color,
              checkbox_box_border_width,
              checkbox_space_between| {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            if !is_finite_style_values(bw, br, fs) {
                return;
            }
            if !exit_history_preview_mode(&ui, &update_style_state) {
                return;
            }

            let mut pm = update_style_state.project_manager.borrow_mut();
            let Some(project) = pm.current_project_mut() else {
                return;
            };
            let before_snapshot = capture_project_snapshot(project, &update_style_state);

            let Some(element_id) = resolve_target_element_id(id.as_str(), &update_style_state)
            else {
                return;
            };
            if !project.update_element_style_on_active_page(
                element_id,
                bg.as_str(),
                border.as_str(),
                bw,
                br,
                fs,
                tc.as_str(),
                ff.as_str(),
                tw.as_str(),
                ph.as_str(),
                bg_image.as_str(),
                image_src.as_str(),
                inherit_text_style,
                checked,
                checkbox_box_side.as_str(),
                checkbox_check_color.as_str(),
                checkbox_box_color.as_str(),
                checkbox_box_border_color.as_str(),
                checkbox_box_border_width,
                checkbox_space_between,
            ) {
                return;
            }

            helpers::save_project_silent(project);

            if let (Some(before_snapshot), Some(after_snapshot)) = (
                before_snapshot,
                capture_project_snapshot(project, &update_style_state),
            ) {
                update_style_state.history.borrow_mut().record_change(
                    HistoryActionKind::ModifyObject,
                    "Edit object style",
                    "Object visual style updated",
                    before_snapshot,
                    after_snapshot,
                );
            }
            refresh_canvas(&ui, project, &update_style_state);
        },
    );

    let ui_weak = ui.as_weak();
    let update_container_settings_state = state.clone();
    ui.on_update_element_container_settings_internal(
        move |id,
              container_mode,
              allow_absolute_children,
              layout_padding,
              layout_padding_left,
              layout_padding_right,
              layout_padding_top,
              layout_padding_bottom,
              layout_spacing,
              layout_margin,
              layout_margin_left,
              layout_margin_right,
              layout_margin_top,
              layout_margin_bottom,
              layout_order,
              stack_alignment,
              flex_direction,
              flex_wrap,
              justify_items,
              justify_content,
              align_items,
              align_content,
              place_items,
              flex_flow,
              grid_template_columns,
              grid_template_rows,
              grid_template_areas| {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            if !(layout_padding.is_finite()
                && layout_padding_left.is_finite()
                && layout_padding_right.is_finite()
                && layout_padding_top.is_finite()
                && layout_padding_bottom.is_finite()
                && layout_spacing.is_finite()
                && layout_margin.is_finite()
                && layout_margin_left.is_finite()
                && layout_margin_right.is_finite()
                && layout_margin_top.is_finite()
                && layout_margin_bottom.is_finite()
                && layout_order.is_finite())
            {
                return;
            }
            if !exit_history_preview_mode(&ui, &update_container_settings_state) {
                return;
            }

            let mut pm = update_container_settings_state.project_manager.borrow_mut();
            let Some(project) = pm.current_project_mut() else {
                return;
            };
            let before_snapshot =
                capture_project_snapshot(project, &update_container_settings_state);

            let Some(element_id) =
                resolve_target_element_id(id.as_str(), &update_container_settings_state)
            else {
                return;
            };
            if !project.update_element_container_settings_on_active_page(
                element_id,
                container_mode.as_str(),
                allow_absolute_children,
                layout_padding,
                layout_padding_left,
                layout_padding_right,
                layout_padding_top,
                layout_padding_bottom,
                layout_spacing,
                layout_margin,
                layout_margin_left,
                layout_margin_right,
                layout_margin_top,
                layout_margin_bottom,
                layout_order,
                stack_alignment.as_str(),
                flex_direction.as_str(),
                flex_wrap.as_str(),
                justify_items.as_str(),
                justify_content.as_str(),
                align_items.as_str(),
                align_content.as_str(),
                place_items.as_str(),
                flex_flow.as_str(),
                grid_template_columns.as_str(),
                grid_template_rows.as_str(),
                grid_template_areas.as_str(),
            ) {
                return;
            }

            helpers::save_project_silent(project);

            if let (Some(before_snapshot), Some(after_snapshot)) = (
                before_snapshot,
                capture_project_snapshot(project, &update_container_settings_state),
            ) {
                update_container_settings_state
                    .history
                    .borrow_mut()
                    .record_change(
                        HistoryActionKind::ModifyObject,
                        "Edit container settings",
                        "Updated layout container constraints",
                        before_snapshot,
                        after_snapshot,
                    );
            }
            refresh_canvas(&ui, project, &update_container_settings_state);
        },
    );

    let ui_weak = ui.as_weak();
    let browse_image_source_state = state.clone();
    ui.on_browse_image_source_internal(move |id| {
        let Some(ui) = ui_weak.upgrade() else {
            return;
        };
        if !exit_history_preview_mode(&ui, &browse_image_source_state) {
            return;
        }

        let Some(path) = pick_image_file() else {
            return;
        };

        let mut pm = browse_image_source_state.project_manager.borrow_mut();
        let Some(project) = pm.current_project_mut() else {
            return;
        };
        let before_snapshot = capture_project_snapshot(project, &browse_image_source_state);

        let Some(element_id) = resolve_target_element_id(id.as_str(), &browse_image_source_state)
        else {
            return;
        };
        let Some(imported_path) = import_image_file_to_project_assets(project, &path) else {
            return;
        };
        if !project.set_element_image_source_on_active_page(element_id, &imported_path) {
            return;
        }

        helpers::save_project_silent(project);

        if let (Some(before_snapshot), Some(after_snapshot)) = (
            before_snapshot,
            capture_project_snapshot(project, &browse_image_source_state),
        ) {
            browse_image_source_state
                .history
                .borrow_mut()
                .record_change(
                    HistoryActionKind::ModifyObject,
                    "Set image source",
                    "Updated image source from file",
                    before_snapshot,
                    after_snapshot,
                );
        }
        refresh_canvas(&ui, project, &browse_image_source_state);
    });
}

fn register_comment_callbacks(ui: &AppWindow, state: &EditorState) {
    let ui_weak = ui.as_weak();
    let create_comment_state = state.clone();
    ui.on_create_comment_internal(move |x, y| {
        let Some(ui) = ui_weak.upgrade() else {
            return;
        };
        if !(x.is_finite() && y.is_finite()) {
            return;
        }
        if !exit_history_preview_mode(&ui, &create_comment_state) {
            return;
        }

        let mut pm = create_comment_state.project_manager.borrow_mut();
        let Some(project) = pm.current_project_mut() else {
            return;
        };
        let before_snapshot = capture_project_snapshot(project, &create_comment_state);

        if project.add_comment_to_active_page(x, y).is_none() {
            return;
        }

        helpers::save_project_silent(project);

        if let (Some(before_snapshot), Some(after_snapshot)) = (
            before_snapshot,
            capture_project_snapshot(project, &create_comment_state),
        ) {
            create_comment_state.history.borrow_mut().record_change(
                HistoryActionKind::CreateObject,
                "Create comment",
                "Added page comment",
                before_snapshot,
                after_snapshot,
            );
        }
        refresh_canvas(&ui, project, &create_comment_state);
    });

    let ui_weak = ui.as_weak();
    let update_comment_content_state = state.clone();
    ui.on_update_comment_content_internal(move |comment_id, title, body| {
        let Some(ui) = ui_weak.upgrade() else {
            return;
        };
        if !exit_history_preview_mode(&ui, &update_comment_content_state) {
            return;
        }

        let mut pm = update_comment_content_state.project_manager.borrow_mut();
        let Some(project) = pm.current_project_mut() else {
            return;
        };
        let before_snapshot = capture_project_snapshot(project, &update_comment_content_state);

        let Some(comment_uuid) = helpers::parse_uuid(comment_id.as_str()) else {
            return;
        };
        if !project.update_comment_content_on_active_page(
            comment_uuid,
            title.as_str(),
            body.as_str(),
        ) {
            return;
        }

        helpers::save_project_silent(project);

        if let (Some(before_snapshot), Some(after_snapshot)) = (
            before_snapshot,
            capture_project_snapshot(project, &update_comment_content_state),
        ) {
            update_comment_content_state
                .history
                .borrow_mut()
                .record_change(
                    HistoryActionKind::ModifyObject,
                    "Edit comment",
                    "Updated comment title or body",
                    before_snapshot,
                    after_snapshot,
                );
        }

        let ui_for_refresh = ui.as_weak();
        let state_for_refresh = update_comment_content_state.clone();
        slint::Timer::single_shot(std::time::Duration::ZERO, move || {
            let Some(ui) = ui_for_refresh.upgrade() else {
                return;
            };
            let mut pm = state_for_refresh.project_manager.borrow_mut();
            let Some(project) = pm.current_project_mut() else {
                return;
            };
            refresh_canvas(&ui, project, &state_for_refresh);
        });
    });

    let ui_weak = ui.as_weak();
    let update_comment_position_state = state.clone();
    ui.on_update_comment_position_internal(move |comment_id, x, y| {
        let Some(ui) = ui_weak.upgrade() else {
            return;
        };
        if !(x.is_finite() && y.is_finite()) {
            return;
        }
        if !exit_history_preview_mode(&ui, &update_comment_position_state) {
            return;
        }

        let mut pm = update_comment_position_state.project_manager.borrow_mut();
        let Some(project) = pm.current_project_mut() else {
            return;
        };
        let before_snapshot = capture_project_snapshot(project, &update_comment_position_state);

        let Some(comment_uuid) = helpers::parse_uuid(comment_id.as_str()) else {
            return;
        };
        if !project.update_comment_position_on_active_page(comment_uuid, x, y) {
            return;
        }

        helpers::save_project_silent(project);

        if let (Some(before_snapshot), Some(after_snapshot)) = (
            before_snapshot,
            capture_project_snapshot(project, &update_comment_position_state),
        ) {
            update_comment_position_state
                .history
                .borrow_mut()
                .record_change(
                    HistoryActionKind::ModifyObject,
                    "Move comment",
                    "Updated comment position",
                    before_snapshot,
                    after_snapshot,
                );
        }
        refresh_canvas(&ui, project, &update_comment_position_state);
    });

    let ui_weak_for_size = ui.as_weak();
    let update_comment_size_state = state.clone();
    ui.on_update_comment_size_internal(move |comment_id, width, body_height| {
        let Some(ui) = ui_weak_for_size.upgrade() else {
            return;
        };
        if !(width.is_finite() && body_height.is_finite()) {
            return;
        }
        if !exit_history_preview_mode(&ui, &update_comment_size_state) {
            return;
        }

        let mut pm = update_comment_size_state.project_manager.borrow_mut();
        let Some(project) = pm.current_project_mut() else {
            return;
        };
        let before_snapshot = capture_project_snapshot(project, &update_comment_size_state);

        let Some(comment_uuid) = helpers::parse_uuid(comment_id.as_str()) else {
            return;
        };
        if !project.update_comment_size_on_active_page(comment_uuid, width, body_height) {
            return;
        }

        helpers::save_project_silent(project);

        if let (Some(before_snapshot), Some(after_snapshot)) = (
            before_snapshot,
            capture_project_snapshot(project, &update_comment_size_state),
        ) {
            update_comment_size_state
                .history
                .borrow_mut()
                .record_change(
                    HistoryActionKind::ModifyObject,
                    "Resize comment",
                    "Updated comment frame size",
                    before_snapshot,
                    after_snapshot,
                );
        }
        refresh_canvas(&ui, project, &update_comment_size_state);
    });

    let ui_weak = ui.as_weak();
    let update_comment_font_sizes_state = state.clone();
    ui.on_update_comment_font_sizes_internal(move |comment_id, title_font_size, body_font_size| {
        let Some(ui) = ui_weak.upgrade() else {
            return;
        };
        if !(title_font_size.is_finite() && body_font_size.is_finite()) {
            return;
        }
        if !exit_history_preview_mode(&ui, &update_comment_font_sizes_state) {
            return;
        }

        let mut pm = update_comment_font_sizes_state.project_manager.borrow_mut();
        let Some(project) = pm.current_project_mut() else {
            return;
        };
        let before_snapshot = capture_project_snapshot(project, &update_comment_font_sizes_state);

        let Some(comment_uuid) = helpers::parse_uuid(comment_id.as_str()) else {
            return;
        };
        if !project.update_comment_font_sizes_on_active_page(
            comment_uuid,
            title_font_size,
            body_font_size,
        ) {
            return;
        }

        helpers::save_project_silent(project);

        if let (Some(before_snapshot), Some(after_snapshot)) = (
            before_snapshot,
            capture_project_snapshot(project, &update_comment_font_sizes_state),
        ) {
            update_comment_font_sizes_state
                .history
                .borrow_mut()
                .record_change(
                    HistoryActionKind::ModifyObject,
                    "Resize comment text",
                    "Updated comment typography",
                    before_snapshot,
                    after_snapshot,
                );
        }
        refresh_canvas(&ui, project, &update_comment_font_sizes_state);
    });

    let ui_weak = ui.as_weak();
    let delete_comment_state = state.clone();
    ui.on_delete_comment_internal(move |comment_id| {
        let Some(ui) = ui_weak.upgrade() else {
            return;
        };
        if !exit_history_preview_mode(&ui, &delete_comment_state) {
            return;
        }

        let mut pm = delete_comment_state.project_manager.borrow_mut();
        let Some(project) = pm.current_project_mut() else {
            return;
        };
        let before_snapshot = capture_project_snapshot(project, &delete_comment_state);

        let Some(comment_uuid) = helpers::parse_uuid(comment_id.as_str()) else {
            return;
        };
        if !project.remove_comment_on_active_page(comment_uuid) {
            return;
        }

        helpers::save_project_silent(project);

        if let (Some(before_snapshot), Some(after_snapshot)) = (
            before_snapshot,
            capture_project_snapshot(project, &delete_comment_state),
        ) {
            delete_comment_state.history.borrow_mut().record_change(
                HistoryActionKind::DeleteObject,
                "Delete comment",
                "Removed page comment",
                before_snapshot,
                after_snapshot,
            );
        }
        refresh_canvas(&ui, project, &delete_comment_state);
    });

    let ui_weak = ui.as_weak();
    let paste_comment_image_state = state.clone();
    ui.on_paste_comment_image_internal(move |comment_id| {
        let Some(ui) = ui_weak.upgrade() else {
            return;
        };
        if !exit_history_preview_mode(&ui, &paste_comment_image_state) {
            return;
        }

        let Some(comment_uuid) = helpers::parse_uuid(comment_id.as_str()) else {
            return;
        };

        let Ok(mut clipboard) = Clipboard::new() else {
            eprintln!("Failed to access clipboard for comment image paste");
            return;
        };
        let Ok(image) = clipboard.get_image() else {
            eprintln!("Clipboard does not contain a compatible image");
            return;
        };

        let width = image.width as u32;
        let height = image.height as u32;
        let rgba = image.bytes.into_owned();

        let mut pm = paste_comment_image_state.project_manager.borrow_mut();
        let Some(project) = pm.current_project_mut() else {
            return;
        };
        let before_snapshot = capture_project_snapshot(project, &paste_comment_image_state);

        let Some(imported_path) =
            save_clipboard_image_to_project_assets(project, width, height, &rgba)
        else {
            return;
        };

        if !project.set_comment_image_on_active_page(comment_uuid, &imported_path, width, height) {
            return;
        }

        helpers::save_project_silent(project);

        if let (Some(before_snapshot), Some(after_snapshot)) = (
            before_snapshot,
            capture_project_snapshot(project, &paste_comment_image_state),
        ) {
            paste_comment_image_state
                .history
                .borrow_mut()
                .record_change(
                    HistoryActionKind::ModifyObject,
                    "Paste comment image",
                    "Attached image from clipboard",
                    before_snapshot,
                    after_snapshot,
                );
        }
        refresh_canvas(&ui, project, &paste_comment_image_state);
    });

    let ui_weak = ui.as_weak();
    let clear_comment_image_state = state.clone();
    ui.on_clear_comment_image_internal(move |comment_id| {
        let Some(ui) = ui_weak.upgrade() else {
            return;
        };
        if !exit_history_preview_mode(&ui, &clear_comment_image_state) {
            return;
        }

        let mut pm = clear_comment_image_state.project_manager.borrow_mut();
        let Some(project) = pm.current_project_mut() else {
            return;
        };
        let before_snapshot = capture_project_snapshot(project, &clear_comment_image_state);

        let Some(comment_uuid) = helpers::parse_uuid(comment_id.as_str()) else {
            return;
        };
        if !project.clear_comment_image_on_active_page(comment_uuid) {
            return;
        }

        helpers::save_project_silent(project);

        if let (Some(before_snapshot), Some(after_snapshot)) = (
            before_snapshot,
            capture_project_snapshot(project, &clear_comment_image_state),
        ) {
            clear_comment_image_state
                .history
                .borrow_mut()
                .record_change(
                    HistoryActionKind::ModifyObject,
                    "Delete comment image",
                    "Removed comment image",
                    before_snapshot,
                    after_snapshot,
                );
        }
        refresh_canvas(&ui, project, &clear_comment_image_state);
    });
}

fn register_hotkey_callbacks(ui: &AppWindow, state: &EditorState) {
    let ui_weak = ui.as_weak();
    let select_all_state = state.clone();
    ui.on_select_all_internal(move || {
        let mut pm = select_all_state.project_manager.borrow_mut();
        let Some(project) = pm.current_project_mut() else {
            return;
        };

        let all_ids: Vec<Uuid> = project
            .active_page_elements()
            .into_iter()
            .map(|element| element.id)
            .collect();
        *select_all_state.selected_elements.borrow_mut() = all_ids;

        if let Some(ui) = ui_weak.upgrade() {
            refresh_canvas(&ui, project, &select_all_state);
        }
    });

    let ui_weak = ui.as_weak();
    let outline_prev_state = state.clone();
    ui.on_select_outline_prev_internal(move || {
        let mut pm = outline_prev_state.project_manager.borrow_mut();
        let Some(project) = pm.current_project_mut() else {
            return;
        };

        let Some(target_id) = select_outline_neighbor(project, &outline_prev_state, -1) else {
            return;
        };
        select_only(&outline_prev_state, target_id);

        if let Some(ui) = ui_weak.upgrade() {
            refresh_canvas(&ui, project, &outline_prev_state);
        }
    });

    let ui_weak = ui.as_weak();
    let outline_next_state = state.clone();
    ui.on_select_outline_next_internal(move || {
        let mut pm = outline_next_state.project_manager.borrow_mut();
        let Some(project) = pm.current_project_mut() else {
            return;
        };

        let Some(target_id) = select_outline_neighbor(project, &outline_next_state, 1) else {
            return;
        };
        select_only(&outline_next_state, target_id);

        if let Some(ui) = ui_weak.upgrade() {
            refresh_canvas(&ui, project, &outline_next_state);
        }
    });

    let copy_state = state.clone();
    ui.on_copy_internal(move || {
        let mut pm = copy_state.project_manager.borrow_mut();
        let Some(project) = pm.current_project_mut() else {
            return;
        };

        let selected_ids = copy_state.selected_elements.borrow().clone();
        copy_state
            .clipboard
            .borrow_mut()
            .copy_from_selection(project, &selected_ids);
    });

    let ui_weak = ui.as_weak();
    let cut_state = state.clone();
    ui.on_cut_internal(move || {
        let Some(ui) = ui_weak.upgrade() else {
            return;
        };
        if !exit_history_preview_mode(&ui, &cut_state) {
            return;
        }

        let mut pm = cut_state.project_manager.borrow_mut();
        let Some(project) = pm.current_project_mut() else {
            return;
        };

        let selected_ids = cut_state.selected_elements.borrow().clone();
        let copied = cut_state
            .clipboard
            .borrow_mut()
            .copy_from_selection(project, &selected_ids);
        if copied == 0 {
            return;
        }

        let Some(before_snapshot) = capture_project_snapshot(project, &cut_state) else {
            return;
        };

        let mut changed = false;
        for id in selected_root_ids(project, &selected_ids) {
            changed |= project.remove_element_on_active_page(id);
        }
        if !changed {
            return;
        }

        helpers::save_project_silent(project);
        retain_existing_selection(project, &cut_state);

        if let Some(after_snapshot) = capture_project_snapshot(project, &cut_state) {
            cut_state.history.borrow_mut().record_change(
                HistoryActionKind::Cut,
                "Cut object",
                format!("Cut objects: {copied}"),
                before_snapshot,
                after_snapshot,
            );
        }
        refresh_canvas(&ui, project, &cut_state);
    });

    let ui_weak = ui.as_weak();
    let paste_state = state.clone();
    ui.on_paste_internal(move |cursor_x, cursor_y| {
        let Some(ui) = ui_weak.upgrade() else {
            return;
        };
        if !exit_history_preview_mode(&ui, &paste_state) {
            return;
        }

        let mut pm = paste_state.project_manager.borrow_mut();
        let Some(project) = pm.current_project_mut() else {
            return;
        };

        if try_paste_clipboard_image_into_selected_image(&ui, project, &paste_state) {
            return;
        }

        let Some(before_snapshot) = capture_project_snapshot(project, &paste_state) else {
            return;
        };
        let (page_w, page_h) = project.active_page_size();
        let target = if cursor_x.is_finite() && cursor_y.is_finite() {
            Some((
                cursor_x.clamp(0.0, page_w as f32),
                cursor_y.clamp(0.0, page_h as f32),
            ))
        } else {
            None
        };
        let pasted_ids = paste_state.clipboard.borrow_mut().paste_into_project_at(
            project,
            target.map(|(x, _)| x),
            target.map(|(_, y)| y),
        );
        if pasted_ids.is_empty() {
            return;
        }

        *paste_state.selected_elements.borrow_mut() = pasted_ids.clone();
        helpers::save_project_silent(project);

        if let Some(after_snapshot) = capture_project_snapshot(project, &paste_state) {
            paste_state.history.borrow_mut().record_change(
                HistoryActionKind::Paste,
                "Paste object",
                format!("Pasted objects: {}", pasted_ids.len()),
                before_snapshot,
                after_snapshot,
            );
        }
        refresh_canvas(&ui, project, &paste_state);
    });

    let ui_weak = ui.as_weak();
    let paste_replace_state = state.clone();
    ui.on_paste_replace_internal(move |cursor_x, cursor_y| {
        let Some(ui) = ui_weak.upgrade() else {
            return;
        };
        if !exit_history_preview_mode(&ui, &paste_replace_state) {
            return;
        }

        if paste_replace_state.clipboard.borrow().is_empty() {
            return;
        }

        let mut pm = paste_replace_state.project_manager.borrow_mut();
        let Some(project) = pm.current_project_mut() else {
            return;
        };

        let selected_snapshot = paste_replace_state.selected_elements.borrow().clone();
        let delete_targets = selected_root_ids(project, &selected_snapshot);
        if delete_targets.is_empty() {
            return;
        }

        let Some(before_snapshot) = capture_project_snapshot(project, &paste_replace_state) else {
            return;
        };

        let fallback_target = selection_center(project, &delete_targets);
        let (page_w, page_h) = project.active_page_size();
        let target = if cursor_x.is_finite() && cursor_y.is_finite() {
            Some((
                cursor_x.clamp(0.0, page_w as f32),
                cursor_y.clamp(0.0, page_h as f32),
            ))
        } else {
            fallback_target
        };

        let mut removed_count = 0usize;
        for element_id in delete_targets {
            if project.remove_element_on_active_page(element_id) {
                removed_count += 1;
            }
        }
        if removed_count == 0 {
            return;
        }

        let pasted_ids = paste_replace_state
            .clipboard
            .borrow_mut()
            .paste_into_project_at(project, target.map(|(x, _)| x), target.map(|(_, y)| y));
        if pasted_ids.is_empty() {
            let _ = restore_project_snapshot(project, &paste_replace_state, &before_snapshot);
            refresh_canvas(&ui, project, &paste_replace_state);
            return;
        }

        *paste_replace_state.selected_elements.borrow_mut() = pasted_ids.clone();
        helpers::save_project_silent(project);

        if let Some(after_snapshot) = capture_project_snapshot(project, &paste_replace_state) {
            paste_replace_state.history.borrow_mut().record_change(
                HistoryActionKind::Paste,
                "Paste replace",
                format!(
                    "Replaced objects: {removed_count}, inserted objects: {}",
                    pasted_ids.len()
                ),
                before_snapshot,
                after_snapshot,
            );
        }
        refresh_canvas(&ui, project, &paste_replace_state);
    });

    let ui_weak = ui.as_weak();
    let flip_vertical_state = state.clone();
    ui.on_flip_vertical_internal(move || {
        let Some(ui) = ui_weak.upgrade() else {
            return;
        };
        if !exit_history_preview_mode(&ui, &flip_vertical_state) {
            return;
        }

        let mut pm = flip_vertical_state.project_manager.borrow_mut();
        let Some(project) = pm.current_project_mut() else {
            return;
        };

        let Some(before_snapshot) = capture_project_snapshot(project, &flip_vertical_state) else {
            return;
        };
        let changed = flip_selected_elements(project, &flip_vertical_state, false);
        if changed == 0 {
            return;
        }

        helpers::save_project_silent(project);
        if let Some(after_snapshot) = capture_project_snapshot(project, &flip_vertical_state) {
            flip_vertical_state.history.borrow_mut().record_change(
                HistoryActionKind::ModifyObject,
                "Flip vertical",
                format!("Flipped objects vertically: {changed}"),
                before_snapshot,
                after_snapshot,
            );
        }
        refresh_canvas(&ui, project, &flip_vertical_state);
    });

    let ui_weak = ui.as_weak();
    let flip_horizontal_state = state.clone();
    ui.on_flip_horizontal_internal(move || {
        let Some(ui) = ui_weak.upgrade() else {
            return;
        };
        if !exit_history_preview_mode(&ui, &flip_horizontal_state) {
            return;
        }

        let mut pm = flip_horizontal_state.project_manager.borrow_mut();
        let Some(project) = pm.current_project_mut() else {
            return;
        };

        let Some(before_snapshot) = capture_project_snapshot(project, &flip_horizontal_state)
        else {
            return;
        };
        let changed = flip_selected_elements(project, &flip_horizontal_state, true);
        if changed == 0 {
            return;
        }

        helpers::save_project_silent(project);
        if let Some(after_snapshot) = capture_project_snapshot(project, &flip_horizontal_state) {
            flip_horizontal_state.history.borrow_mut().record_change(
                HistoryActionKind::ModifyObject,
                "Flip horizontal",
                format!("Flipped objects horizontally: {changed}"),
                before_snapshot,
                after_snapshot,
            );
        }
        refresh_canvas(&ui, project, &flip_horizontal_state);
    });

    let ui_weak = ui.as_weak();
    let undo_state = state.clone();
    ui.on_undo_internal(move || {
        let Some(ui) = ui_weak.upgrade() else {
            return;
        };
        if !exit_history_preview_mode(&ui, &undo_state) {
            return;
        }

        let Some(snapshot) = undo_state.history.borrow_mut().undo() else {
            return;
        };

        let mut pm = undo_state.project_manager.borrow_mut();
        let Some(project) = pm.current_project_mut() else {
            return;
        };
        if !restore_project_snapshot(project, &undo_state, &snapshot) {
            return;
        }

        helpers::save_project_silent(project);
        sync::sync_editor_models(&ui, project);
        apply_scene_for_active_document(&ui, project);
        refresh_canvas(&ui, project, &undo_state);
    });

    let ui_weak = ui.as_weak();
    let redo_state = state.clone();
    ui.on_redo_internal(move || {
        let Some(ui) = ui_weak.upgrade() else {
            return;
        };
        if !exit_history_preview_mode(&ui, &redo_state) {
            return;
        }

        let Some(snapshot) = redo_state.history.borrow_mut().redo() else {
            return;
        };

        let mut pm = redo_state.project_manager.borrow_mut();
        let Some(project) = pm.current_project_mut() else {
            return;
        };
        if !restore_project_snapshot(project, &redo_state, &snapshot) {
            return;
        }

        helpers::save_project_silent(project);
        sync::sync_editor_models(&ui, project);
        apply_scene_for_active_document(&ui, project);
        refresh_canvas(&ui, project, &redo_state);
    });

    let ui_weak = ui.as_weak();
    let exit_project_state = state.clone();
    ui.on_exit_project_internal(move || {
        let Some(ui) = ui_weak.upgrade() else {
            return;
        };
        {
            let mut pm = exit_project_state.project_manager.borrow_mut();
            if let Some(project) = pm.current_project_mut() {
                save_project_history_silent(project, &exit_project_state);
            }
        }
        exit_project_state.history.borrow_mut().reset();
        exit_project_state.clipboard.borrow_mut().clear();
        clear_selection_and_outline(&exit_project_state);
        sync::sync_timeline(&ui, &exit_project_state.history.borrow());
        ui.set_current_scene(AppScene::ProjectSelected);
    });

    let ui_weak = ui.as_weak();
    let timeline_state = state.clone();
    ui.on_timeline_entry_selected_internal(move |index| {
        let Some(ui) = ui_weak.upgrade() else {
            return;
        };

        let mut pm = timeline_state.project_manager.borrow_mut();
        let Some(project) = pm.current_project_mut() else {
            return;
        };

        let Some(live_snapshot) = capture_project_snapshot(project, &timeline_state) else {
            return;
        };

        let selection = timeline_state
            .history
            .borrow_mut()
            .select_timeline_entry(index.max(0) as usize, live_snapshot);

        let snapshot = match selection {
            PreviewSelection::Apply(snapshot) | PreviewSelection::Restore(snapshot) => snapshot,
            PreviewSelection::None => {
                sync::sync_timeline(&ui, &timeline_state.history.borrow());
                return;
            }
        };

        if !restore_project_snapshot(project, &timeline_state, &snapshot) {
            return;
        }
        sync::sync_editor_models(&ui, project);
        apply_scene_for_active_document(&ui, project);
        refresh_canvas(&ui, project, &timeline_state);
    });

    let ui_weak = ui.as_weak();
    let exit_preview_state = state.clone();
    ui.on_exit_history_preview_internal(move || {
        let Some(ui) = ui_weak.upgrade() else {
            return;
        };
        let _ = exit_history_preview_mode(&ui, &exit_preview_state);
    });
}

fn register_search_helpers(ui: &AppWindow) {
    ui.global::<StringSearch>().on_contains(|text, search| {
        if search.is_empty() {
            return true;
        }
        text.to_lowercase().contains(&search.to_lowercase())
    });

    ui.global::<StringSearch>()
        .on_contains_any(|components, search| {
            if search.is_empty() {
                return true;
            }

            let search_lower = search.to_lowercase();
            for component in components.iter() {
                if component.to_lowercase().contains(&search_lower) {
                    return true;
                }
            }
            false
        });
}

fn load_project_history_silent(project: &Project, state: &EditorState) {
    let mut history = state.history.borrow_mut();
    history.reset();
    match operations::load_project_history(&project.spx_file_path()) {
        Ok(Some(bytes)) => {
            if let Err(err) = history.load_from_bytes(&bytes) {
                eprintln!("Failed to decode project history: {err}");
                history.reset();
            }
        }
        Ok(None) => {}
        Err(err) => {
            eprintln!("Failed to load project history: {err}");
            history.reset();
        }
    }
}

fn save_project_history_silent(project: &Project, state: &EditorState) {
    let bytes: Vec<u8> = match state.history.borrow().save_to_bytes() {
        Ok(bytes) => bytes,
        Err(err) => {
            eprintln!("Failed to encode project history: {err}");
            return;
        }
    };
    let path = project.spx_file_path();
    std::thread::spawn(move || {
        if let Err(err) = operations::save_project_history(&path, &bytes) {
            eprintln!("Failed to save project history: {err}");
        }
    });
}

fn apply_scene_for_active_document(ui: &AppWindow, project: &Project) {
    let scene = match project.active_document_kind() {
        Some(crate::app::project::EditorDocumentKind::PageBlueprint)
        | Some(crate::app::project::EditorDocumentKind::ServerBlueprint) => {
            AppScene::BlueprintEditor
        }
        _ => AppScene::VisualEditor,
    };
    ui.set_current_scene(scene);
}

fn activate_project(ui: &AppWindow, project: &Project, state: &EditorState) {
    clear_selection_and_outline(state);
    state.clipboard.borrow_mut().clear();
    load_project_history_silent(project, state);
    sync::sync_editor_models(ui, project);
    refresh_canvas(ui, project, state);
    ui.set_project_name(SharedString::from(project.name()));
    apply_scene_for_active_document(ui, project);
}

fn load_project_from_path(ui: &AppWindow, state: &EditorState, path: &str) {
    let mut pm = state.project_manager.borrow_mut();
    match pm.load_project(path) {
        Ok(project) => {
            let mut recent_storage = config::RecentProjectsStorage::load();
            recent_storage.add_project(project.name(), path);
            activate_project(ui, project, state);
            sync::set_recent_projects(ui, &recent_storage);
        }
        Err(err) => {
            eprintln!("Failed to load project: {err}");
            app_errors::report(AppErrorCode::ProjectLoadFailed);
        }
    }
}

fn relocate_project_path(current_path: &str, project_name: &str) -> Option<String> {
    let current_path = current_path.trim();
    if current_path.is_empty() {
        return FileDialog::new()
            .set_title("Select Project File")
            .add_filter("Snappix Project", &["spx"])
            .pick_file()
            .map(|path| path.to_string_lossy().to_string());
    }

    let source = PathBuf::from(current_path);
    if !source.exists() {
        app_errors::report(AppErrorCode::ProjectRelocateFailed);
        return None;
    }

    let destination_folder = FileDialog::new()
        .set_title("Select New Project Location")
        .pick_folder()?;

    let file_name = source
        .file_name()
        .map(|value| value.to_owned())
        .unwrap_or_else(|| {
            std::ffi::OsString::from(format!("{}.spx", sanitize_name(project_name)))
        });
    let destination = destination_folder.join(file_name);

    if source == destination {
        return Some(source.to_string_lossy().to_string());
    }

    if let Err(err) = std::fs::rename(&source, &destination) {
        eprintln!(
            "Failed to move project file from {} to {}: {err}",
            source.display(),
            destination.display()
        );
        app_errors::report(AppErrorCode::ProjectRelocateFailed);
        return None;
    }

    Some(destination.to_string_lossy().to_string())
}

fn delete_project_from_disk(path: &str, project_name: &str) -> Result<(), std::io::Error> {
    let path = path.trim();
    if path.is_empty() {
        // Nothing to delete on disk, but recent entry should still be removable.
        return Ok(());
    }

    let target = PathBuf::from(path);
    if !target.exists() {
        // Project may have already been deleted manually.
        return Ok(());
    }

    if target.is_file() {
        std::fs::remove_file(target)?;
        return Ok(());
    }

    if target.is_dir() {
        // Safety: do not remove an arbitrary directory recursively.
        // We only delete matching .spx file(s) inside the selected directory.
        let mut removed_any = false;

        if !project_name.trim().is_empty() {
            let named_candidate = target.join(format!("{}.spx", project_name.trim()));
            if named_candidate.exists() && named_candidate.is_file() {
                std::fs::remove_file(named_candidate)?;
                removed_any = true;
            }
        }

        if !removed_any {
            let mut spx_files = Vec::new();
            for entry in std::fs::read_dir(&target)? {
                let entry = entry?;
                let entry_path = entry.path();
                let is_spx = entry_path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| ext.eq_ignore_ascii_case("spx"))
                    .unwrap_or(false);
                if entry_path.is_file() && is_spx {
                    spx_files.push(entry_path);
                }
            }

            // Delete only when we can identify a single project file unambiguously.
            if spx_files.len() == 1 {
                std::fs::remove_file(&spx_files[0])?;
                removed_any = true;
            }
        }

        if removed_any && target.read_dir()?.next().is_none() {
            // Best-effort cleanup of an empty project directory.
            let _ = std::fs::remove_dir(&target);
        }

        return Ok(());
    }

    Ok(())
}

fn sanitize_name(name: &str) -> String {
    let mut sanitized = String::new();
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == ' ' {
            sanitized.push(ch);
        } else {
            sanitized.push('_');
        }
    }

    let trimmed = sanitized.trim();
    if trimmed.is_empty() {
        "project".to_string()
    } else {
        trimmed.to_string()
    }
}

fn pick_image_file() -> Option<PathBuf> {
    FileDialog::new()
        .set_title("Select Image")
        .add_filter("Images", &["png", "jpg", "jpeg", "webp"])
        .pick_file()
}

fn project_images_dir(project: &Project) -> PathBuf {
    project.assets_root_dir().join("assets").join("images")
}

fn project_image_asset_path(file_name: &str) -> String {
    Path::new("assets")
        .join("images")
        .join(file_name)
        .to_string_lossy()
        .replace('\\', "/")
}

fn import_image_file_to_project_assets(project: &Project, source: &Path) -> Option<String> {
    let extension = source
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())?;
    if !matches!(extension.as_str(), "png" | "jpg" | "jpeg" | "webp") {
        return None;
    }

    let destination_dir = project_images_dir(project);
    if let Err(err) = std::fs::create_dir_all(&destination_dir) {
        eprintln!(
            "Failed to create image assets directory {}: {err}",
            destination_dir.display()
        );
        return None;
    }

    let file_name = format!("{}.{}", Uuid::new_v4(), extension);
    let destination = destination_dir.join(&file_name);
    if let Err(err) = std::fs::copy(source, &destination) {
        eprintln!(
            "Failed to import image {} into project assets {}: {err}",
            source.display(),
            destination.display()
        );
        return None;
    }

    Some(project_image_asset_path(&file_name))
}

fn save_clipboard_image_to_project_assets(
    project: &Project,
    width: u32,
    height: u32,
    rgba: &[u8],
) -> Option<String> {
    let image = RgbaImage::from_raw(width, height, rgba.to_vec())?;

    let destination_dir = project_images_dir(project);
    if let Err(err) = std::fs::create_dir_all(&destination_dir) {
        eprintln!(
            "Failed to create clipboard image assets directory {}: {err}",
            destination_dir.display()
        );
        return None;
    }

    let file_name = format!("{}.png", Uuid::new_v4());
    let destination = destination_dir.join(&file_name);
    if let Err(err) = image.save_with_format(&destination, ImageFormat::Png) {
        eprintln!(
            "Failed to save clipboard image into project assets {}: {err}",
            destination.display()
        );
        return None;
    }

    Some(project_image_asset_path(&file_name))
}

fn try_paste_clipboard_image_into_selected_image(
    ui: &AppWindow,
    project: &mut Project,
    state: &EditorState,
) -> bool {
    let selected_id = {
        let selected = state.selected_elements.borrow();
        if selected.len() != 1 {
            return false;
        }
        selected[0]
    };

    let Some(element) = project.get_element_on_active_page(selected_id) else {
        return false;
    };
    if element.element_type != "image" {
        return false;
    }

    let Ok(mut clipboard) = Clipboard::new() else {
        return false;
    };
    let Ok(image) = clipboard.get_image() else {
        return false;
    };

    let width = image.width as u32;
    let height = image.height as u32;
    let rgba = image.bytes.into_owned();
    let Some(before_snapshot) = capture_project_snapshot(project, state) else {
        return false;
    };
    let Some(imported_path) = save_clipboard_image_to_project_assets(project, width, height, &rgba)
    else {
        return false;
    };
    if !project.set_element_image_source_on_active_page(selected_id, &imported_path) {
        return false;
    }

    helpers::save_project_silent(project);

    if let Some(after_snapshot) = capture_project_snapshot(project, state) {
        state.history.borrow_mut().record_change(
            HistoryActionKind::ModifyObject,
            "Paste image source",
            "Updated selected image from clipboard",
            before_snapshot,
            after_snapshot,
        );
    }
    refresh_canvas(ui, project, state);
    true
}

fn refresh_canvas(ui: &AppWindow, project: &Project, state: &EditorState) {
    sync::sync_canvas(
        ui,
        project,
        &state.selected_elements.borrow(),
        &state.collapsed_outline_nodes.borrow(),
    );
    sync::sync_timeline(ui, &state.history.borrow());
}

fn refresh_canvas_preview(ui: &AppWindow, project: &Project, state: &EditorState) {
    sync::sync_canvas_view(ui, project, &state.selected_elements.borrow(), None);
}

fn clear_selection_and_outline(state: &EditorState) {
    state.selected_elements.borrow_mut().clear();
    state.collapsed_outline_nodes.borrow_mut().clear();
    state.transform_preview.borrow_mut().take();
}

fn select_only(state: &EditorState, element_id: Uuid) {
    let mut selected = state.selected_elements.borrow_mut();
    selected.clear();
    selected.push(element_id);
}

fn ensure_primary_selection(state: &EditorState, element_id: Uuid) {
    let mut selected = state.selected_elements.borrow_mut();
    if selected.is_empty() {
        selected.push(element_id);
    } else if !selected.contains(&element_id) {
        selected.clear();
        selected.push(element_id);
    }
}

fn resolve_target_element_id(raw_id: &str, state: &EditorState) -> Option<Uuid> {
    helpers::parse_uuid(raw_id).or_else(|| state.selected_elements.borrow().first().copied())
}

fn retain_existing_selection(project: &Project, state: &EditorState) {
    let existing: HashSet<Uuid> = project
        .active_page_elements()
        .into_iter()
        .map(|element| element.id)
        .collect();
    state
        .selected_elements
        .borrow_mut()
        .retain(|selected_id| existing.contains(selected_id));
}

fn capture_project_snapshot(project: &Project, state: &EditorState) -> Option<ProjectSnapshot> {
    let selected = state.selected_elements.borrow().clone();
    let collapsed = state.collapsed_outline_nodes.borrow().clone();
    ProjectSnapshot::capture(project, &selected, &collapsed)
}

fn restore_project_snapshot(
    project: &mut Project,
    state: &EditorState,
    snapshot: &ProjectSnapshot,
) -> bool {
    if !snapshot.restore_project(project) {
        return false;
    }

    let existing_ids: HashSet<Uuid> = project
        .active_page_elements()
        .into_iter()
        .map(|element| element.id)
        .collect();

    {
        let mut selected = state.selected_elements.borrow_mut();
        selected.clear();
        for id in &snapshot.selected_elements {
            if existing_ids.contains(id) && !selected.contains(id) {
                selected.push(*id);
            }
        }
    }

    {
        let mut collapsed = state.collapsed_outline_nodes.borrow_mut();
        collapsed.clear();
        for id in &snapshot.collapsed_outline_nodes {
            if existing_ids.contains(id) {
                collapsed.insert(*id);
            }
        }
    }

    state.transform_preview.borrow_mut().take();
    true
}

fn exit_history_preview_mode(ui: &AppWindow, state: &EditorState) -> bool {
    let snapshot = state.history.borrow_mut().exit_preview();
    let Some(snapshot) = snapshot else {
        sync::sync_timeline(ui, &state.history.borrow());
        return true;
    };

    let mut pm = state.project_manager.borrow_mut();
    let Some(project) = pm.current_project_mut() else {
        return false;
    };

    if !restore_project_snapshot(project, state, &snapshot) {
        return false;
    }

    sync::sync_editor_models(ui, project);
    apply_scene_for_active_document(ui, project);
    refresh_canvas(ui, project, state);
    true
}

fn selected_root_ids(project: &Project, selected_ids: &[Uuid]) -> Vec<Uuid> {
    let selected_set: HashSet<Uuid> = selected_ids.iter().copied().collect();
    let mut roots = Vec::new();

    for selected_id in selected_ids {
        let mut parent = project.element_parent_on_active_page(*selected_id);
        let mut has_selected_ancestor = false;
        while let Some(parent_id) = parent {
            if selected_set.contains(&parent_id) {
                has_selected_ancestor = true;
                break;
            }
            parent = project.element_parent_on_active_page(parent_id);
        }

        if !has_selected_ancestor && !roots.contains(selected_id) {
            roots.push(*selected_id);
        }
    }

    roots
}

fn select_outline_neighbor(project: &Project, state: &EditorState, step: i32) -> Option<Uuid> {
    let outline_order =
        sync::visible_outline_order(project, &state.collapsed_outline_nodes.borrow());
    if outline_order.is_empty() {
        return None;
    }

    let current_selected = state.selected_elements.borrow().first().copied();
    let fallback = if step >= 0 {
        outline_order[0]
    } else {
        *outline_order.last().unwrap_or(&outline_order[0])
    };

    let Some(current_selected) = current_selected else {
        return Some(fallback);
    };
    let Some(current_index) = outline_order.iter().position(|id| *id == current_selected) else {
        return Some(fallback);
    };

    let target_index = if step < 0 {
        current_index.saturating_sub(1)
    } else {
        (current_index + 1).min(outline_order.len() - 1)
    };
    outline_order.get(target_index).copied()
}

fn move_selected_group(
    project: &mut Project,
    selected_snapshot: &[Uuid],
    dx: f32,
    dy: f32,
) -> bool {
    if dx.abs() <= 0.001 && dy.abs() <= 0.001 {
        return false;
    }

    let move_roots = selected_root_ids(project, selected_snapshot);
    let mut root_elements = Vec::with_capacity(move_roots.len());
    let mut group_min_x = f32::INFINITY;
    let mut group_min_y = f32::INFINITY;
    let mut group_max_x = f32::NEG_INFINITY;
    let mut group_max_y = f32::NEG_INFINITY;

    for root_id in move_roots {
        let Some(root_element) = project.get_element_on_active_page(root_id) else {
            continue;
        };
        let (min_x, min_y, max_x, max_y) = rotated_selection_bounds(&root_element);
        group_min_x = group_min_x.min(min_x);
        group_min_y = group_min_y.min(min_y);
        group_max_x = group_max_x.max(max_x);
        group_max_y = group_max_y.max(max_y);
        root_elements.push((root_id, root_element));
    }

    if root_elements.is_empty() {
        return false;
    }

    let clamp_delta = |requested: f32, min_allowed: f32, max_allowed: f32| -> f32 {
        if min_allowed > max_allowed {
            0.0
        } else {
            requested.max(min_allowed).min(max_allowed)
        }
    };
    let (page_w, page_h) = project.active_page_size();
    let page_w = page_w as f32;
    let page_h = page_h as f32;
    let bounded_dx = clamp_delta(dx, -group_min_x, page_w - group_max_x);
    let bounded_dy = clamp_delta(dy, -group_min_y, page_h - group_max_y);

    if bounded_dx.abs() <= 0.001 && bounded_dy.abs() <= 0.001 {
        return false;
    }

    let mut changed = false;
    for (root_id, root_element) in root_elements {
        changed |= project.update_element_geometry_on_active_page(
            root_id,
            root_element.x + bounded_dx,
            root_element.y + bounded_dy,
            root_element.width,
            root_element.height,
            root_element.rotation,
        );
    }

    changed
}

fn apply_geometry_update(
    project: &mut Project,
    selected_snapshot: &[Uuid],
    element_id: Uuid,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    r: f32,
) -> bool {
    if let Some(current) = project.get_element_on_active_page(element_id) {
        let move_only = (w - current.width).abs() <= 0.001
            && (h - current.height).abs() <= 0.001
            && (r - current.rotation).abs() <= 0.001;

        if move_only && selected_snapshot.len() > 1 && selected_snapshot.contains(&element_id) {
            let dx = x - current.x;
            let dy = y - current.y;
            return move_selected_group(project, selected_snapshot, dx, dy);
        }
    }

    let (page_w, page_h) = project.active_page_size();
    let (x, y, width, height, rotation) =
        helpers::clamp_rotated_geometry_to_page(x, y, w, h, r, page_w as f32, page_h as f32);

    let mut changed =
        project.update_element_geometry_on_active_page(element_id, x, y, width, height, rotation);

    if let Some(parent_id) = project.managed_layout_parent_on_active_page(element_id) {
        changed |= project.relayout_container_on_active_page(parent_id);
    }
    if project.container_uses_managed_layout_on_active_page(element_id) {
        changed |= project.relayout_container_on_active_page(element_id);
    }

    changed
}

fn capture_transform_preview(project: &Project, state: &EditorState, element_id: Uuid) {
    let mut preview = state.transform_preview.borrow_mut();
    let should_refresh_snapshot = preview
        .as_ref()
        .map(|current| current.element_id != element_id)
        .unwrap_or(true);
    if should_refresh_snapshot {
        *preview = Some(TransformPreviewState {
            element_id,
            geometries: project.snapshot_element_geometries_on_active_page(),
        });
    }
}

fn has_transform_preview(state: &EditorState, element_id: Uuid) -> bool {
    state
        .transform_preview
        .borrow()
        .as_ref()
        .map(|preview| preview.element_id == element_id)
        .unwrap_or(false)
}

fn take_transform_preview(state: &EditorState, element_id: Uuid) -> Option<TransformPreviewState> {
    let mut preview = state.transform_preview.borrow_mut();
    let matches = preview
        .as_ref()
        .map(|current| current.element_id == element_id)
        .unwrap_or(false);
    if matches {
        preview.take()
    } else {
        None
    }
}

fn selection_center(project: &Project, element_ids: &[Uuid]) -> Option<(f32, f32)> {
    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_y = f32::NEG_INFINITY;

    for element_id in element_ids {
        let Some(element) = project.get_element_on_active_page(*element_id) else {
            continue;
        };
        let (bx1, by1, bx2, by2) = rotated_selection_bounds(&element);
        min_x = min_x.min(bx1);
        min_y = min_y.min(by1);
        max_x = max_x.max(bx2);
        max_y = max_y.max(by2);
    }

    if !(min_x.is_finite() && min_y.is_finite() && max_x.is_finite() && max_y.is_finite()) {
        return None;
    }

    Some((min_x + (max_x - min_x) / 2.0, min_y + (max_y - min_y) / 2.0))
}

fn normalize_rotation_degrees(rotation: f32) -> f32 {
    let mut normalized = (rotation + 180.0).rem_euclid(360.0) - 180.0;
    if normalized <= -180.0 {
        normalized += 360.0;
    }
    normalized
}

fn flip_selected_elements(project: &mut Project, state: &EditorState, horizontal: bool) -> usize {
    let selected_snapshot = state.selected_elements.borrow().clone();
    let root_ids = selected_root_ids(project, &selected_snapshot);
    if root_ids.is_empty() {
        return 0;
    }

    let Some((axis_x, axis_y)) = selection_center(project, &root_ids) else {
        return 0;
    };

    let (page_w, page_h) = project.active_page_size();
    let page_w = page_w as f32;
    let page_h = page_h as f32;

    let mut changed = 0usize;
    for element_id in root_ids {
        let Some(element) = project.get_element_on_active_page(element_id) else {
            continue;
        };

        let center_x = element.x + element.width / 2.0;
        let center_y = element.y + element.height / 2.0;
        let next_center_x = if horizontal {
            2.0 * axis_x - center_x
        } else {
            center_x
        };
        let next_center_y = if horizontal {
            center_y
        } else {
            2.0 * axis_y - center_y
        };

        let next_x = next_center_x - element.width / 2.0;
        let next_y = next_center_y - element.height / 2.0;
        let next_rotation = normalize_rotation_degrees(element.rotation);

        let (clamped_x, clamped_y, clamped_w, clamped_h, clamped_rotation) =
            helpers::clamp_rotated_geometry_to_page(
                next_x,
                next_y,
                element.width,
                element.height,
                next_rotation,
                page_w,
                page_h,
            );

        let geometry_changed = project.update_element_geometry_on_active_page(
            element_id,
            clamped_x,
            clamped_y,
            clamped_w,
            clamped_h,
            clamped_rotation,
        );
        let flip_changed = project.toggle_element_flip_on_active_page(element_id, horizontal);
        if geometry_changed || flip_changed {
            changed += 1;
        }
    }

    changed
}

fn rotated_selection_bounds(
    element: &crate::app::project::CanvasElementData,
) -> (f32, f32, f32, f32) {
    let radians = element.rotation.to_radians();
    let abs_cos = radians.cos().abs();
    let abs_sin = radians.sin().abs();
    let bbox_w = element.width * abs_cos + element.height * abs_sin;
    let bbox_h = element.width * abs_sin + element.height * abs_cos;
    let center_x = element.x + element.width / 2.0;
    let center_y = element.y + element.height / 2.0;
    (
        center_x - bbox_w / 2.0,
        center_y - bbox_h / 2.0,
        center_x + bbox_w / 2.0,
        center_y + bbox_h / 2.0,
    )
}

fn rects_intersect(
    a_min_x: f32,
    a_min_y: f32,
    a_max_x: f32,
    a_max_y: f32,
    b_min_x: f32,
    b_min_y: f32,
    b_max_x: f32,
    b_max_y: f32,
) -> bool {
    a_max_x >= b_min_x && a_min_x <= b_max_x && a_max_y >= b_min_y && a_min_y <= b_max_y
}

fn is_finite_geometry(x: f32, y: f32, w: f32, h: f32, r: f32) -> bool {
    x.is_finite() && y.is_finite() && w.is_finite() && h.is_finite() && r.is_finite()
}

fn is_finite_style_values(border_width: f32, border_radius: f32, font_size: f32) -> bool {
    border_width.is_finite() && border_radius.is_finite() && font_size.is_finite()
}

fn parse_platform(index: i32) -> Platform {
    match index {
        1 => Platform::Mobile,
        2 => Platform::Web,
        _ => Platform::Desktop,
    }
}

fn parse_dev_mode(index: i32) -> DevMode {
    match index {
        0 => DevMode::Code,
        2 => DevMode::Hybrid,
        _ => DevMode::Nodes,
    }
}

fn parse_page_size(index: i32) -> PageSize {
    match index {
        1 => PageSize::Tablet,
        2 => PageSize::Mobile,
        3 => PageSize::Custom,
        _ => PageSize::Desktop,
    }
}
