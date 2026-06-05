use std::collections::{HashMap, HashSet};

use uuid::Uuid;

use crate::{CanvasElementData, Project};

#[derive(Debug, Clone)]
struct ClipboardEntry {
    source_id: Uuid,
    parent_id: Option<Uuid>,
    element: CanvasElementData,
}

#[derive(Debug, Clone, Default)]
pub struct ClipboardBuffer {
    entries: Vec<ClipboardEntry>,
    paste_generation: u32,
}

impl ClipboardBuffer {
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.paste_generation = 0;
    }

    pub fn copy_from_selection(&mut self, project: &Project, selected_ids: &[Uuid]) -> usize {
        let all_elements = project.active_page_elements();
        let element_map: HashMap<Uuid, CanvasElementData> = all_elements
            .into_iter()
            .map(|element| (element.id, element))
            .collect();

        let mut seen = HashSet::new();
        let mut ordered_ids = Vec::new();
        for selected_id in selected_ids {
            if element_map.contains_key(selected_id) && seen.insert(*selected_id) {
                ordered_ids.push(*selected_id);
            }
        }
        if ordered_ids.is_empty() {
            self.clear();
            return 0;
        }

        let selected_set: HashSet<Uuid> = ordered_ids.iter().copied().collect();
        ordered_ids.sort_by_key(|id| ancestor_depth(*id, &element_map, &selected_set));

        let mut entries = Vec::new();
        for id in ordered_ids {
            let Some(element) = element_map.get(&id).cloned() else {
                continue;
            };
            let parent_id =
                element_parent_id(&element).filter(|parent| selected_set.contains(parent));
            entries.push(ClipboardEntry {
                source_id: id,
                parent_id,
                element,
            });
        }

        let copied = entries.len();
        if copied > 0 {
            self.entries = entries;
            self.paste_generation = 0;
        } else {
            self.clear();
        }
        copied
    }

    pub fn paste_into_project_at(
        &mut self,
        project: &mut Project,
        cursor_x: Option<f32>,
        cursor_y: Option<f32>,
    ) -> Vec<Uuid> {
        if self.entries.is_empty() {
            return Vec::new();
        }

        let (offset_x, offset_y) = if let (Some(cursor_x), Some(cursor_y)) = (cursor_x, cursor_y) {
            let (min_x, min_y, max_x, max_y) = self.entries.iter().fold(
                (f32::MAX, f32::MAX, f32::MIN, f32::MIN),
                |(min_x, min_y, max_x, max_y), entry| {
                    (
                        min_x.min(entry.element.x),
                        min_y.min(entry.element.y),
                        max_x.max(entry.element.x + entry.element.width),
                        max_y.max(entry.element.y + entry.element.height),
                    )
                },
            );
            // Place pasted selection around the cursor (center anchored), not at copy origin.
            let source_center_x = min_x + (max_x - min_x) / 2.0;
            let source_center_y = min_y + (max_y - min_y) / 2.0;
            (cursor_x - source_center_x, cursor_y - source_center_y)
        } else {
            self.paste_generation = self.paste_generation.saturating_add(1);
            let offset = (self.paste_generation as f32) * 20.0;
            (offset, offset)
        };

        let mut id_map = HashMap::new();
        for entry in &self.entries {
            id_map.insert(entry.source_id, Uuid::new_v4());
        }

        let mut pasted_ids = Vec::new();
        for entry in &self.entries {
            let Some(new_id) = id_map.get(&entry.source_id).copied() else {
                continue;
            };

            let mut element = entry.element.clone();
            element.id = new_id;
            element.x += offset_x;
            element.y += offset_y;

            let parent_id = entry
                .parent_id
                .and_then(|parent| id_map.get(&parent).copied());
            set_element_parent_id(&mut element, parent_id);

            if let Some(added_id) =
                project.add_element_to_active_page_with_parent(element, parent_id)
            {
                pasted_ids.push(added_id);
            }
        }

        pasted_ids
    }
}

fn element_parent_id(element: &CanvasElementData) -> Option<Uuid> {
    element
        .properties
        .as_object()
        .and_then(|props| props.get("parent_id"))
        .and_then(|value| value.as_str())
        .and_then(|text| Uuid::parse_str(text).ok())
}

fn set_element_parent_id(element: &mut CanvasElementData, parent_id: Option<Uuid>) {
    let Some(props) = element.properties.as_object_mut() else {
        return;
    };
    match parent_id {
        Some(parent_id) => {
            props.insert(
                "parent_id".to_string(),
                serde_json::json!(parent_id.to_string()),
            );
        }
        None => {
            props.remove("parent_id");
        }
    }
}

fn ancestor_depth(
    id: Uuid,
    element_map: &HashMap<Uuid, CanvasElementData>,
    selected_set: &HashSet<Uuid>,
) -> usize {
    let mut depth = 0usize;
    let mut cursor = id;
    let mut guard = 0usize;
    while guard < 256 {
        guard += 1;
        let Some(parent_id) = element_map
            .get(&cursor)
            .and_then(element_parent_id)
            .filter(|parent| selected_set.contains(parent))
        else {
            break;
        };
        depth += 1;
        cursor = parent_id;
    }
    depth
}
