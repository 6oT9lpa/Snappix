use std::collections::{HashMap, HashSet};

use uuid::Uuid;

use crate::{CanvasElementData, Project};

pub fn selected_root_ids(project: &Project, selected_ids: &[Uuid]) -> Vec<Uuid> {
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

pub fn selected_and_descendant_ids(project: &Project, selected_ids: &[Uuid]) -> Vec<Uuid> {
    let all_elements = project.active_page_elements();
    let existing: HashSet<Uuid> = all_elements.iter().map(|element| element.id).collect();
    let mut children_map: HashMap<Uuid, Vec<Uuid>> = HashMap::new();

    for element in &all_elements {
        let parent_id = element
            .properties
            .as_object()
            .and_then(|props| props.get("parent_id"))
            .and_then(|value| value.as_str())
            .and_then(|value| Uuid::parse_str(value).ok())
            .filter(|parent_id| *parent_id != element.id && existing.contains(parent_id));
        if let Some(parent_id) = parent_id {
            children_map.entry(parent_id).or_default().push(element.id);
        }
    }

    let mut out = Vec::new();
    let mut visited = HashSet::new();
    for selected_id in selected_ids {
        collect_descendants_for_toggle(
            *selected_id,
            &existing,
            &children_map,
            &mut visited,
            &mut out,
        );
    }
    out
}

fn collect_descendants_for_toggle(
    element_id: Uuid,
    existing: &HashSet<Uuid>,
    children_map: &HashMap<Uuid, Vec<Uuid>>,
    visited: &mut HashSet<Uuid>,
    out: &mut Vec<Uuid>,
) {
    if !existing.contains(&element_id) || !visited.insert(element_id) {
        return;
    }
    out.push(element_id);
    if let Some(children) = children_map.get(&element_id) {
        for child_id in children {
            collect_descendants_for_toggle(*child_id, existing, children_map, visited, out);
        }
    }
}

pub fn selection_center(project: &Project, element_ids: &[Uuid]) -> Option<(f32, f32)> {
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

pub fn normalize_rotation_degrees(rotation: f32) -> f32 {
    let mut normalized = (rotation + 180.0).rem_euclid(360.0) - 180.0;
    if normalized <= -180.0 {
        normalized += 360.0;
    }
    normalized
}

pub fn rotated_selection_bounds(element: &CanvasElementData) -> (f32, f32, f32, f32) {
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

pub fn rects_intersect(
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

pub fn is_finite_geometry(x: f32, y: f32, w: f32, h: f32, r: f32) -> bool {
    x.is_finite() && y.is_finite() && w.is_finite() && h.is_finite() && r.is_finite()
}

pub fn is_finite_style_values(border_width: f32, border_radius: f32, font_size: f32) -> bool {
    border_width.is_finite() && border_radius.is_finite() && font_size.is_finite()
}
