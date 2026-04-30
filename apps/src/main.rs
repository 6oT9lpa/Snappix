#![cfg_attr(all(target_os = "windows", not(test)), windows_subsystem = "windows")]

use slint::ComponentHandle;
use slint::SharedString;
use std::error::Error;
use std::path::PathBuf;

mod app;
mod app_errors;
mod config;
mod editor_runtime;

slint::include_modules!();

fn main() -> Result<(), Box<dyn Error>> {
    let launch_project = project_path_from_cli_args();
    let ui = AppWindow::new()?;
    let editor_state = editor_runtime::EditorState::new();
    app_errors::attach_ui(&ui);

    editor_runtime::configure_app_settings(&ui);
    editor_runtime::configure_hotkeys(&ui);
    editor_runtime::load_recent_projects(&ui);
    editor_runtime::register_callbacks(&ui, &editor_state);

    app::associations::register_file_associations();
    app::setup_navigation_handler(&ui);
    app::auth::setup_login_handler(&ui);
    app::auth::setup_logout_handler(&ui);
    if app::auth::try_auto_login(&ui) {
        ui.invoke_authenticated();
    }

    if let Some(project_path) = launch_project {
        ui.invoke_load_project_internal(SharedString::from(project_path));
    }

    ui.run()?;
    Ok(())
}

fn project_path_from_cli_args() -> Option<String> {
    std::env::args_os().skip(1).find_map(|arg| {
        let mut raw = arg.to_string_lossy().trim().to_string();
        if raw.is_empty() || raw.starts_with("-psn_") {
            return None;
        }

        // Finder/desktop integrations may pass file:// URI-style arguments.
        if let Some(rest) = raw.strip_prefix("file:///") {
            raw = if cfg!(target_os = "windows") {
                rest.replace('/', "\\")
            } else {
                format!("/{}", rest)
            };
        } else if let Some(rest) = raw.strip_prefix("file://") {
            raw = rest.to_string();
        }

        let candidate = PathBuf::from(raw);
        let is_spx = candidate
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("spx"))
            .unwrap_or(false);

        if !is_spx {
            return None;
        }

        Some(candidate.to_string_lossy().to_string())
    })
}
