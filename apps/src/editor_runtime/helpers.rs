use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use uuid::Uuid;

use crate::app::project::{CanvasElementData, Project};
use crate::app_errors::{self, AppErrorCode};

const AUTOSAVE_INTERVAL: Duration = Duration::from_secs(30);
const LOGGER_TARGET: &str = "apps.editor_runtime.helpers";

static LAST_PROJECT_SAVE: OnceLock<Mutex<HashMap<PathBuf, Instant>>> = OnceLock::new();

pub fn clamp_rotated_geometry_to_page(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    rotation: f32,
    page_w: f32,
    page_h: f32,
) -> (f32, f32, f32, f32, f32) {
    let page_w = page_w.max(1.0);
    let page_h = page_h.max(1.0);

    let mut width = width.max(1.0).min(page_w);
    let mut height = height.max(1.0).min(page_h);
    let rotation = rotation.clamp(-180.0, 180.0);

    let (bbox_w, bbox_h) = rotated_bounding_box(width, height, rotation);
    let scale = (page_w / bbox_w.max(1.0))
        .min(page_h / bbox_h.max(1.0))
        .min(1.0);
    width = (width * scale).max(1.0).min(page_w);
    height = (height * scale).max(1.0).min(page_h);

    let (bbox_w, bbox_h) = rotated_bounding_box(width, height, rotation);
    let min_cx = bbox_w / 2.0;
    let max_cx = (page_w - bbox_w / 2.0).max(min_cx);
    let min_cy = bbox_h / 2.0;
    let max_cy = (page_h - bbox_h / 2.0).max(min_cy);

    let center_x = (x + width / 2.0).clamp(min_cx, max_cx);
    let center_y = (y + height / 2.0).clamp(min_cy, max_cy);

    let x = center_x - width / 2.0;
    let y = center_y - height / 2.0;

    (x, y, width, height, rotation)
}

pub fn clamp_element_to_page(project: &Project, element: &mut CanvasElementData) {
    let (page_w, page_h) = project.active_page_size();
    let (x, y, width, height, rotation) = clamp_rotated_geometry_to_page(
        element.x,
        element.y,
        element.width,
        element.height,
        element.rotation,
        page_w as f32,
        page_h as f32,
    );

    element.x = x;
    element.y = y;
    element.width = width;
    element.height = height;
    element.rotation = rotation;
}

pub fn find_drop_parent(project: &Project, canvas_x: f32, canvas_y: f32) -> Option<Uuid> {
    let elements = project.active_page_elements();
    for element in elements.iter().rev() {
        let is_container = matches!(
            element.element_type.as_str(),
            "div"
                | "stack-container"
                | "stack"
                | "flex-container"
                | "flex"
                | "grid-container"
                | "grid"
        );
        if !is_container {
            continue;
        }
        let within_x = canvas_x >= element.x && canvas_x <= element.x + element.width;
        let within_y = canvas_y >= element.y && canvas_y <= element.y + element.height;
        if within_x && within_y {
            return Some(element.id);
        }
    }
    None
}

pub fn parse_uuid(text: &str) -> Option<Uuid> {
    Uuid::parse_str(text).ok()
}

pub fn save_project_silent(project: &Project) {
    save_project_autosave(project, "autosave");
}

pub fn save_project_forced(project: &Project, reason: &str) {
    save_project_now(project, reason, true);
}

fn save_project_autosave(project: &Project, reason: &str) {
    let path = project.spx_file_path();

    // UI callbacks can fire many times during drag/selection updates. Throttle
    // autosave here so callers do not need to remember whether they are noisy.
    if let Some(elapsed) = autosave_elapsed_since_last_save(&path) {
        if elapsed < AUTOSAVE_INTERVAL {
            shared::log_trace!(
                LOGGER_TARGET,
                "Autosave skipped: path='{}', reason='{}', elapsed_ms={}, next_allowed_ms={}",
                path.display(),
                reason,
                elapsed.as_millis(),
                AUTOSAVE_INTERVAL.saturating_sub(elapsed).as_millis()
            );
            return;
        }
    }

    save_project_now(project, reason, false);
}

fn save_project_now(project: &Project, reason: &str, forced: bool) {
    let path = project.spx_file_path();
    shared::log_info!(
        LOGGER_TARGET,
        "Project save requested: path='{}', reason='{}', forced={}",
        path.display(),
        reason,
        forced
    );
    if let Err(err) = project.save() {
        shared::log_error!(
            LOGGER_TARGET,
            "Project save failed: path='{}', reason='{}', error='{}'",
            path.display(),
            reason,
            err
        );
        app_errors::report(AppErrorCode::ProjectSaveFailed);
    } else {
        mark_project_saved(path.as_path());
        shared::log_info!(
            LOGGER_TARGET,
            "Project saved: path='{}', reason='{}', forced={}",
            path.display(),
            reason,
            forced
        );
    }
}

fn autosave_elapsed_since_last_save(path: &Path) -> Option<Duration> {
    let saves = LAST_PROJECT_SAVE.get_or_init(|| Mutex::new(HashMap::new()));
    let saves = saves
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    saves.get(path).map(Instant::elapsed)
}

fn mark_project_saved(path: &Path) {
    let saves = LAST_PROJECT_SAVE.get_or_init(|| Mutex::new(HashMap::new()));
    saves
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(path.to_path_buf(), Instant::now());
}

fn rotated_bounding_box(width: f32, height: f32, rotation_deg: f32) -> (f32, f32) {
    let radians = rotation_deg.to_radians();
    let abs_cos = radians.cos().abs();
    let abs_sin = radians.sin().abs();
    (
        width * abs_cos + height * abs_sin,
        width * abs_sin + height * abs_cos,
    )
}
