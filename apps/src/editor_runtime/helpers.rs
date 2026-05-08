use shared::{log_fields, LogCategory, LogLevel, LogMessage};
use uuid::Uuid;

use crate::app::project::{CanvasElementData, Project};
use crate::app_errors::{self, AppErrorCode};

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
    log_fields(
        LogLevel::Debug,
        LogCategory::Project,
        LogMessage::ProjectSaveRequested,
        [(
            "path",
            project.spx_file_path().to_string_lossy().to_string(),
        )],
    );
    if let Err(err) = project.save() {
        log_fields(
            LogLevel::Error,
            LogCategory::Project,
            LogMessage::ProjectSaveFailed,
            [("error", err.to_string())],
        );
        app_errors::report(AppErrorCode::ProjectSaveFailed);
    } else {
        log_fields(
            LogLevel::Debug,
            LogCategory::Project,
            LogMessage::ProjectSaved,
            [(
                "path",
                project.spx_file_path().to_string_lossy().to_string(),
            )],
        );
    }
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
