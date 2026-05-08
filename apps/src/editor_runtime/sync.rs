use slint::{Color, Image, SharedString, VecModel};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;
use uuid::Uuid;

use core_blueprint::{
    builtin_node_descriptor, BlueprintNodeKind, BlueprintPinDirection, BlueprintPinKind,
    BlueprintPinType,
};

use crate::app::project::{CanvasElementData, Project};
use crate::config;
use crate::editor_runtime::history::HistoryManager;
use crate::{
    AppWindow, BlueprintFunctionInfo, BlueprintLinkInfo, BlueprintNodeInfo, BlueprintVariableInfo,
    CanvasBounds, CanvasCommentInfo, CanvasPosition, CanvasSize, EditorDocumentInfo,
    ProjectPageInfo, RecentProjectData, SelectionInfo, TimelineEntry,
};

#[derive(Clone)]
struct ResolvedTextStyle {
    font_size: f32,
    text_color: Color,
    font_family: String,
    text_wrap: String,
}

pub fn load_recent_projects(ui: &AppWindow) {
    let storage = config::RecentProjectsStorage::load();
    set_recent_projects(ui, &storage);
}

pub fn set_recent_projects(ui: &AppWindow, storage: &config::RecentProjectsStorage) {
    let recent: Vec<RecentProjectData> = storage
        .get_all()
        .iter()
        .map(|project| RecentProjectData {
            name: SharedString::from(&project.name),
            path: SharedString::from(&project.path),
            date: SharedString::from(format_date(&project.last_opened)),
        })
        .collect();

    ui.set_recent_projects(Rc::new(VecModel::from(recent)).into());
}

pub fn sync_editor_models(ui: &AppWindow, project: &Project) {
    let page_names = project.page_names();
    let page_count = page_names.len();
    if page_count == 0 {
        ui.set_editor_documents(Rc::new(VecModel::from(Vec::<EditorDocumentInfo>::new())).into());
        ui.set_project_page_items(Rc::new(VecModel::from(Vec::<ProjectPageInfo>::new())).into());
        ui.set_blueprint_variable_items(
            Rc::new(VecModel::from(Vec::<BlueprintVariableInfo>::new())).into(),
        );
        ui.set_blueprint_function_items(
            Rc::new(VecModel::from(Vec::<BlueprintFunctionInfo>::new())).into(),
        );
        ui.set_blueprint_node_items(
            Rc::new(VecModel::from(Vec::<BlueprintNodeInfo>::new())).into(),
        );
        ui.set_blueprint_link_items(
            Rc::new(VecModel::from(Vec::<BlueprintLinkInfo>::new())).into(),
        );
        ui.set_active_document_index(0);
        return;
    }

    let documents: Vec<EditorDocumentInfo> = project
        .open_documents()
        .iter()
        .filter_map(|document| {
            let label = project.document_label(document)?;
            let kind = match project.document_kind(document)? {
                crate::app::project::EditorDocumentKind::PageUi => "page_ui",
                crate::app::project::EditorDocumentKind::PageBlueprint => "page_blueprint",
                crate::app::project::EditorDocumentKind::ServerBlueprint => "server_blueprint",
            };

            Some(EditorDocumentInfo {
                document_id: SharedString::from(match document {
                    project_manager::EditorDocumentRef::PageUi { page_id } => page_id.to_string(),
                    project_manager::EditorDocumentRef::PageBlueprint { document_id }
                    | project_manager::EditorDocumentRef::ServerBlueprint { document_id } => {
                        document_id.to_string()
                    }
                }),
                label: SharedString::from(label),
                kind: SharedString::from(kind),
                linked_page_index: project
                    .linked_page_index_for_document(document)
                    .map(|index| index as i32)
                    .unwrap_or(-1),
                is_active: project.active_document() == Some(document),
                is_dirty: false,
            })
        })
        .collect();
    ui.set_editor_documents(Rc::new(VecModel::from(documents.clone())).into());

    let active_tab_idx = documents
        .iter()
        .position(|document| document.is_active)
        .unwrap_or(0);
    ui.set_active_document_index(active_tab_idx as i32);

    let open_set: HashSet<usize> = project
        .open_documents()
        .iter()
        .filter_map(|document| project.linked_page_index_for_document(document))
        .collect();
    let page_items: Vec<ProjectPageInfo> = page_names
        .iter()
        .enumerate()
        .map(|(idx, name)| ProjectPageInfo {
            page_index: idx as i32,
            page_name: SharedString::from(name.clone()),
            is_open: open_set.contains(&idx),
            is_active: idx == project.active_page_index(),
        })
        .collect();
    ui.set_project_page_items(Rc::new(VecModel::from(page_items)).into());
    sync_blueprint_models(ui, project);

    let (width, height) = project.active_page_size();
    ui.set_page_width(width as f32);
    ui.set_page_height(height as f32);
}

pub fn sync_blueprint_models(ui: &AppWindow, project: &Project) {
    let variables: Vec<BlueprintVariableInfo> = project
        .active_blueprint_local_variables()
        .into_iter()
        .map(|variable| BlueprintVariableInfo {
            variable_id: SharedString::from(variable.id.to_string()),
            name: SharedString::from(variable.name),
            type_name: SharedString::from(variable_type_label(
                variable.data_type,
                variable.item_type,
            )),
            item_type_name: SharedString::from(
                variable.item_type.map(pin_type_label).unwrap_or("String"),
            ),
            value_label: SharedString::from(variable_value_label(variable.value.as_ref())),
            color: pin_type_color(variable.data_type),
        })
        .collect();
    ui.set_blueprint_variable_items(Rc::new(VecModel::from(variables)).into());

    let functions: Vec<BlueprintFunctionInfo> = project
        .active_blueprint_functions()
        .into_iter()
        .map(|function| BlueprintFunctionInfo {
            function_id: SharedString::from(function.name.clone()),
            name: SharedString::from(function.name),
            return_type: SharedString::from(pin_type_label(function.return_type)),
        })
        .collect();
    ui.set_blueprint_function_items(Rc::new(VecModel::from(functions)).into());

    let blueprint_nodes = project.active_blueprint_nodes();
    let blueprint_links = project.active_blueprint_links();
    let exec_output_pin_ids: HashSet<Uuid> = blueprint_nodes
        .iter()
        .flat_map(|node| {
            node.pins.iter().filter_map(|pin| {
                (pin.direction == BlueprintPinDirection::Output
                    && pin.kind == BlueprintPinKind::Exec)
                    .then_some(pin.id)
            })
        })
        .collect();
    let linked_exec_output_pin_ids: HashSet<Uuid> = blueprint_links
        .iter()
        .filter_map(|link| {
            exec_output_pin_ids
                .contains(&link.from_pin_id)
                .then_some(link.from_pin_id)
        })
        .collect();
    let linked_output_pin_ids: HashSet<Uuid> = blueprint_links
        .iter()
        .map(|link| link.from_pin_id)
        .collect();

    let node_exec_connected: HashMap<Uuid, [bool; 3]> = blueprint_nodes
        .iter()
        .map(|node| {
            let exec_output_pin_ids_for_node: Vec<Uuid> = node
                .pins
                .iter()
                .filter(|pin| {
                    pin.direction == BlueprintPinDirection::Output
                        && pin.kind == BlueprintPinKind::Exec
                })
                .map(|pin| pin.id)
                .collect();
            let mut connected = [false, false, false];
            for i in 0..3usize {
                if let Some(pin_id) = exec_output_pin_ids_for_node.get(i) {
                    connected[i] = linked_exec_output_pin_ids.contains(pin_id);
                }
            }
            (node.id, connected)
        })
        .collect();
    let node_layouts: HashMap<Uuid, (f32, f32, f32, f32)> = blueprint_nodes
        .iter()
        .map(|node| {
            let (width, height) = project_core::blueprint_node_visual_size(node);
            (
                node.id,
                (
                    node.position.x as f32,
                    node.position.y as f32,
                    width as f32,
                    height as f32,
                ),
            )
        })
        .collect();

    let nodes: Vec<BlueprintNodeInfo> = blueprint_nodes
        .iter()
        .map(|node| {
            let exec_outputs: Vec<_> = node
                .pins
                .iter()
                .filter(|pin| {
                    pin.direction == BlueprintPinDirection::Output
                        && pin.kind == BlueprintPinKind::Exec
                })
                .map(|pin| pin.name.clone())
                .collect();
            let data_output = node.pins.iter().find(|pin| {
                pin.direction == BlueprintPinDirection::Output && pin.kind == BlueprintPinKind::Data
            });
            let data_input = node.pins.iter().find(|pin| {
                pin.direction == BlueprintPinDirection::Input && pin.kind == BlueprintPinKind::Data
            });
            let has_exec_input = node.pins.iter().any(|pin| {
                pin.direction == BlueprintPinDirection::Input && pin.kind == BlueprintPinKind::Exec
            });

            let (category, element_type, descriptor_id, is_event, is_bound_event) =
                blueprint_node_display_meta(&node.kind);
            BlueprintNodeInfo {
                node_id: SharedString::from(node.id.to_string()),
                title: SharedString::from(node.title.clone()),
                category: SharedString::from(category),
                element_type: SharedString::from(element_type),
                descriptor_id: SharedString::from(descriptor_id),
                x: node.position.x as f32,
                y: node.position.y as f32,
                is_event,
                is_bound_event,
                has_exec_input,
                exec_output_1: SharedString::from(
                    exec_outputs.first().cloned().unwrap_or_default(),
                ),
                exec_output_2: SharedString::from(exec_outputs.get(1).cloned().unwrap_or_default()),
                exec_output_3: SharedString::from(exec_outputs.get(2).cloned().unwrap_or_default()),
                exec_output_1_connected: node_exec_connected
                    .get(&node.id)
                    .map(|value| value[0])
                    .unwrap_or(false),
                exec_output_2_connected: node_exec_connected
                    .get(&node.id)
                    .map(|value| value[1])
                    .unwrap_or(false),
                exec_output_3_connected: node_exec_connected
                    .get(&node.id)
                    .map(|value| value[2])
                    .unwrap_or(false),
                data_output_name: SharedString::from(
                    data_output.map(|pin| pin.name.clone()).unwrap_or_default(),
                ),
                data_output_type: SharedString::from(
                    data_output
                        .map(|pin| pin_type_label(pin.data_type))
                        .unwrap_or(""),
                ),
                data_output_color: data_output
                    .map(|pin| pin_type_color(pin.data_type))
                    .unwrap_or_else(|| Color::from_rgb_u8(90, 198, 139)),
                data_output_connected: data_output
                    .map(|pin| linked_output_pin_ids.contains(&pin.id))
                    .unwrap_or(false),
                data_input_name: SharedString::from(
                    data_input.map(|pin| pin.name.clone()).unwrap_or_default(),
                ),
                data_input_type: SharedString::from(
                    data_input
                        .map(|pin| pin_type_label(pin.data_type))
                        .unwrap_or(""),
                ),
                data_input_color: data_input
                    .map(|pin| pin_type_color(pin.data_type))
                    .unwrap_or_else(|| Color::from_rgb_u8(90, 198, 139)),
            }
        })
        .collect();
    ui.set_blueprint_node_items(Rc::new(VecModel::from(nodes)).into());

    let links: Vec<BlueprintLinkInfo> = blueprint_links
        .into_iter()
        .filter_map(|link| {
            let (from_x, from_y, from_width, _) = *node_layouts.get(&link.from_node_id)?;
            let (to_x, to_y, _, _) = *node_layouts.get(&link.to_node_id)?;
            let from_node = blueprint_nodes
                .iter()
                .find(|node| node.id == link.from_node_id)?;
            let to_node = blueprint_nodes
                .iter()
                .find(|node| node.id == link.to_node_id)?;
            let from_pin = from_node
                .pins
                .iter()
                .find(|pin| pin.id == link.from_pin_id)?;
            let to_pin = to_node.pins.iter().find(|pin| pin.id == link.to_pin_id)?;
            let from_is_event = blueprint_node_uses_compact_event_width(from_node);
            let to_is_event = blueprint_node_uses_compact_event_width(to_node);
            let from_render_width = if from_is_event {
                (from_width - 100.0).max(0.0)
            } else {
                from_width
            };
            let from_exec_output_shift_x = if !from_is_event
                && from_pin.kind == BlueprintPinKind::Exec
                && from_pin.direction == BlueprintPinDirection::Output
            {
                -5.0
            } else {
                0.0
            };
            let to_exec_input_shift_x = if !to_is_event
                && to_pin.kind == BlueprintPinKind::Exec
                && to_pin.direction == BlueprintPinDirection::Input
            {
                5.0
            } else {
                0.0
            };
            let link_color = if from_pin.kind == BlueprintPinKind::Data {
                pin_type_color(from_pin.data_type)
            } else {
                Color::from_rgb_u8(240, 240, 240)
            };
            Some(BlueprintLinkInfo {
                from_node_id: SharedString::from(link.from_node_id.to_string()),
                to_node_id: SharedString::from(link.to_node_id.to_string()),
                from_x: from_x + from_render_width + from_exec_output_shift_x,
                from_y: from_y + node_pin_anchor_offset_y(from_node, link.from_pin_id),
                to_x: to_x + to_exec_input_shift_x,
                to_y: to_y + node_pin_anchor_offset_y(to_node, link.to_pin_id),
                link_color,
            })
        })
        .collect();
    ui.set_blueprint_link_items(Rc::new(VecModel::from(links)).into());
}

fn blueprint_node_uses_compact_event_width(node: &core_blueprint::BlueprintNode) -> bool {
    match &node.kind {
        BlueprintNodeKind::UiEvent { .. } | BlueprintNodeKind::CatalogEvent { .. } => true,
        BlueprintNodeKind::Catalog { descriptor_id } => builtin_node_descriptor(descriptor_id)
            .map(|descriptor| descriptor.category == "Events")
            .unwrap_or(false),
        _ => false,
    }
}

fn node_pin_anchor_offset_y(node: &core_blueprint::BlueprintNode, pin_id: Uuid) -> f32 {
    let Some(pin) = node.pins.iter().find(|pin| pin.id == pin_id) else {
        return 58.0;
    };
    match (pin.direction, pin.kind) {
        (BlueprintPinDirection::Output, BlueprintPinKind::Exec) => {
            let exec_outputs: Vec<Uuid> = node
                .pins
                .iter()
                .filter(|candidate| {
                    candidate.direction == BlueprintPinDirection::Output
                        && candidate.kind == BlueprintPinKind::Exec
                })
                .map(|candidate| candidate.id)
                .collect();
            let index = exec_outputs
                .iter()
                .position(|candidate_id| *candidate_id == pin_id)
                .unwrap_or(0);
            60.0 + index as f32 * 28.0
        }
        (BlueprintPinDirection::Input, BlueprintPinKind::Exec) => 58.0,
        (_, BlueprintPinKind::Data) => {
            if matches!(node.kind, BlueprintNodeKind::VariableGet { .. }) {
                50.0
            } else {
                88.0
            }
        }
    }
}

fn blueprint_node_display_meta(kind: &BlueprintNodeKind) -> (String, String, String, bool, bool) {
    match kind {
        BlueprintNodeKind::UiEvent {
            element_id,
            event_name,
        } => (
            "Events".to_string(),
            "Event Element".to_string(),
            format!("ui_event:{event_name}"),
            true,
            !element_id.is_nil(),
        ),
        BlueprintNodeKind::Catalog { descriptor_id } => {
            let descriptor = builtin_node_descriptor(descriptor_id);
            let category = descriptor
                .as_ref()
                .map(|descriptor| descriptor.category.clone())
                .unwrap_or_else(|| "Nodes".to_string());
            let is_event = category == "Events";
            let element_type = descriptor
                .as_ref()
                .map(|descriptor| descriptor.title.clone())
                .unwrap_or_else(|| "Catalog Node".to_string());
            (
                category,
                element_type,
                descriptor_id.clone(),
                is_event,
                false,
            )
        }
        BlueprintNodeKind::CatalogEvent { descriptor_id, .. } => {
            let descriptor = builtin_node_descriptor(descriptor_id);
            let category = descriptor
                .as_ref()
                .map(|descriptor| descriptor.category.clone())
                .unwrap_or_else(|| "Events".to_string());
            let element_type = descriptor
                .as_ref()
                .map(|descriptor| descriptor.title.clone())
                .unwrap_or_else(|| "Catalog Event".to_string());
            (category, element_type, descriptor_id.clone(), true, true)
        }
        BlueprintNodeKind::VariableGet { .. } => (
            "Variables".to_string(),
            "Variable".to_string(),
            "variable.get".to_string(),
            false,
            false,
        ),
        BlueprintNodeKind::VariableSet { .. } => (
            "Variables".to_string(),
            "Variable".to_string(),
            "variable.set".to_string(),
            false,
            false,
        ),
        BlueprintNodeKind::SetElementText { .. } => (
            "UI".to_string(),
            "Action".to_string(),
            "ui.set_text".to_string(),
            false,
            false,
        ),
        BlueprintNodeKind::LiteralString { .. } => (
            "Values".to_string(),
            "String".to_string(),
            "literal.string".to_string(),
            false,
            false,
        ),
        BlueprintNodeKind::FunctionEntry { .. } => (
            "Functions".to_string(),
            "Function".to_string(),
            "function.entry".to_string(),
            true,
            true,
        ),
        BlueprintNodeKind::FunctionResult { .. } => (
            "Functions".to_string(),
            "Function".to_string(),
            "function.result".to_string(),
            false,
            false,
        ),
        BlueprintNodeKind::CallDocumentFunction { .. } => (
            "Functions".to_string(),
            "Function".to_string(),
            "function.call".to_string(),
            false,
            false,
        ),
        BlueprintNodeKind::Functional { node_id } => (
            "Legacy".to_string(),
            "Functional".to_string(),
            node_id.clone(),
            false,
            false,
        ),
    }
}

fn pin_type_label(pin_type: BlueprintPinType) -> &'static str {
    match pin_type {
        BlueprintPinType::Exec => "exec",
        BlueprintPinType::Any => "any",
        BlueprintPinType::Bool => "bool",
        BlueprintPinType::Int => "int",
        BlueprintPinType::Float => "float",
        BlueprintPinType::String => "String",
        BlueprintPinType::Color => "Color",
        BlueprintPinType::Array => "Array",
        BlueprintPinType::Vector => "Vector",
        BlueprintPinType::HashSet => "Set",
        BlueprintPinType::HashMap => "HashMap",
        BlueprintPinType::Object => "Object",
        BlueprintPinType::UiElementRef => "Element",
        BlueprintPinType::PageRef => "Page",
        BlueprintPinType::ApiRef => "Api",
        BlueprintPinType::Void => "void",
    }
}

fn variable_type_label(data_type: BlueprintPinType, item_type: Option<BlueprintPinType>) -> String {
    let base = pin_type_label(data_type);
    match data_type {
        BlueprintPinType::Array
        | BlueprintPinType::Vector
        | BlueprintPinType::HashSet
        | BlueprintPinType::HashMap => {
            let item = item_type.map(pin_type_label).unwrap_or("String");
            format!("{base}<{item}>")
        }
        _ => base.to_string(),
    }
}

fn pin_type_color(pin_type: BlueprintPinType) -> Color {
    match pin_type {
        BlueprintPinType::Bool => Color::from_rgb_u8(220, 68, 79),
        BlueprintPinType::Int | BlueprintPinType::Float => Color::from_rgb_u8(76, 137, 219),
        BlueprintPinType::String => Color::from_rgb_u8(218, 171, 64),
        BlueprintPinType::Color => Color::from_rgb_u8(181, 93, 205),
        BlueprintPinType::Object => Color::from_rgb_u8(70, 190, 124),
        BlueprintPinType::Array
        | BlueprintPinType::Vector
        | BlueprintPinType::HashSet
        | BlueprintPinType::HashMap => Color::from_rgb_u8(225, 128, 65),
        BlueprintPinType::UiElementRef => Color::from_rgb_u8(62, 168, 137),
        BlueprintPinType::PageRef | BlueprintPinType::ApiRef => Color::from_rgb_u8(70, 137, 145),
        BlueprintPinType::Any => Color::from_rgb_u8(132, 139, 150),
        BlueprintPinType::Exec | BlueprintPinType::Void => Color::from_rgb_u8(235, 235, 235),
    }
}

fn variable_value_label(value: Option<&serde_json::Value>) -> String {
    let Some(value) = value else {
        return String::new();
    };
    value
        .get("name")
        .and_then(|value| value.as_str())
        .filter(|name| !name.trim().is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| value.to_string())
}

pub fn sync_canvas(
    ui: &AppWindow,
    project: &Project,
    selected_ids: &[Uuid],
    collapsed_outline_nodes: &HashSet<Uuid>,
    hidden_element_ids: &HashSet<Uuid>,
) {
    let elements = project.active_page_elements();
    sync_canvas_view(ui, project, selected_ids, Some(&elements), hidden_element_ids);
    sync_canvas_comments(ui, project);
    sync_outline_view(
        ui,
        project,
        selected_ids,
        collapsed_outline_nodes,
        Some(&elements),
    );
}

pub fn sync_canvas_comments(ui: &AppWindow, project: &Project) {
    let comments: Vec<CanvasCommentInfo> = project
        .active_page_comments()
        .into_iter()
        .map(|comment| {
            let (image, image_width, image_height, has_image) = comment
                .image
                .as_ref()
                .map(|image| {
                    let has_image = !image.path.trim().is_empty();
                    let loaded = if has_image {
                        load_image_from_project_path(project, &image.path)
                    } else {
                        Image::default()
                    };
                    (loaded, image.width as i32, image.height as i32, has_image)
                })
                .unwrap_or_else(|| (Image::default(), 0, 0, false));

            CanvasCommentInfo {
                comment_id: SharedString::from(comment.id.to_string()),
                position: CanvasPosition {
                    x: comment.x,
                    y: comment.y,
                },
                width: comment.width,
                body_height: comment.body_height,
                title: SharedString::from(comment.title),
                body: SharedString::from(comment.body),
                title_font_size: comment.title_font_size,
                body_font_size: comment.body_font_size,
                has_image,
                image,
                image_width,
                image_height,
            }
        })
        .collect();

    ui.set_canvas_comments(Rc::new(VecModel::from(comments)).into());
}

pub fn sync_canvas_view(
    ui: &AppWindow,
    project: &Project,
    selected_ids: &[Uuid],
    elements_override: Option<&[CanvasElementData]>,
    hidden_element_ids: &HashSet<Uuid>,
) {
    let owned_elements;
    let elements = if let Some(elements) = elements_override {
        elements
    } else {
        owned_elements = project.active_page_elements();
        &owned_elements
    };

    let element_map: HashMap<Uuid, CanvasElementData> = elements
        .iter()
        .cloned()
        .map(|element| (element.id, element))
        .collect();
    let parent_map = build_parent_map(elements);
    let children_map = build_children_map(elements, &parent_map);
    let resolved_text_styles = resolve_text_styles(&element_map, &parent_map);
    let (selected_list, selected_set, group_anchor) =
        build_selection_state(selected_ids, &element_map);

    let canvas_infos: Vec<SelectionInfo> = elements
        .iter()
        .filter(|element| !hidden_element_ids.contains(&element.id))
        .map(|element| {
            let has_children = children_map
                .get(&element.id)
                .map(|children| !children.is_empty())
                .unwrap_or(false);
            to_selection_info(
                project,
                element,
                &selected_set,
                group_anchor,
                0,
                has_children,
                true,
                resolved_text_styles.get(&element.id),
            )
        })
        .collect();
    ui.set_canvas_elements(Rc::new(VecModel::from(canvas_infos)).into());

    let selected_primary_id = selected_list.first().copied();
    let selected_primary = selected_primary_id.and_then(|id| element_map.get(&id).cloned());
    let selected_style = selected_primary_id.and_then(|id| resolved_text_styles.get(&id));
    set_selected_element(ui, project, selected_primary.as_ref(), selected_style);
}

pub fn sync_outline_view(
    ui: &AppWindow,
    project: &Project,
    selected_ids: &[Uuid],
    collapsed_outline_nodes: &HashSet<Uuid>,
    elements_override: Option<&[CanvasElementData]>,
) {
    let owned_elements;
    let elements = if let Some(elements) = elements_override {
        elements
    } else {
        owned_elements = project.active_page_elements();
        &owned_elements
    };

    let element_map: HashMap<Uuid, CanvasElementData> = elements
        .iter()
        .cloned()
        .map(|element| (element.id, element))
        .collect();
    let parent_map = build_parent_map(elements);
    let children_map = build_children_map(elements, &parent_map);
    let resolved_text_styles = resolve_text_styles(&element_map, &parent_map);
    let (_selected_list, selected_set, group_anchor) =
        build_selection_state(selected_ids, &element_map);

    let order: HashMap<Uuid, usize> = elements
        .iter()
        .enumerate()
        .map(|(idx, element)| (element.id, idx))
        .collect();
    let mut top_level: Vec<Uuid> = elements
        .iter()
        .filter(|element| {
            parent_map
                .get(&element.id)
                .and_then(|parent| *parent)
                .is_none()
        })
        .map(|element| element.id)
        .collect();
    top_level.sort_by_key(|id| order.get(id).copied().unwrap_or(usize::MAX));

    let mut outline_infos = Vec::new();
    let mut visited = HashSet::new();
    for top_id in top_level {
        push_outline(
            project,
            top_id,
            0,
            &element_map,
            &children_map,
            collapsed_outline_nodes,
            &selected_set,
            group_anchor,
            &resolved_text_styles,
            &mut outline_infos,
            &mut visited,
        );
    }
    ui.set_outline_elements(Rc::new(VecModel::from(outline_infos)).into());
}

pub fn sync_timeline(ui: &AppWindow, history: &HistoryManager) {
    let timeline_entries: Vec<TimelineEntry> = history
        .timeline_entries()
        .iter()
        .enumerate()
        .map(|(index, entry)| TimelineEntry {
            index: index as i32,
            title: SharedString::from(entry.title.clone()),
            details: SharedString::from(entry.details.clone()),
            timestamp: SharedString::from(entry.timestamp.clone()),
            action_kind: SharedString::from(entry.action_kind.as_tag()),
        })
        .collect();

    ui.set_timeline_events(Rc::new(VecModel::from(timeline_entries)).into());
    ui.set_active_timeline_index(
        history
            .active_timeline_index()
            .map(|idx| idx as i32)
            .unwrap_or(-1),
    );
    ui.set_history_preview_active(history.is_preview_active());
}

pub fn visible_outline_order(
    project: &Project,
    collapsed_outline_nodes: &HashSet<Uuid>,
) -> Vec<Uuid> {
    let elements = project.active_page_elements();
    let parent_map = build_parent_map(&elements);
    let children_map = build_children_map(&elements, &parent_map);
    let order: HashMap<Uuid, usize> = elements
        .iter()
        .enumerate()
        .map(|(idx, element)| (element.id, idx))
        .collect();
    let mut top_level: Vec<Uuid> = elements
        .iter()
        .filter(|element| {
            parent_map
                .get(&element.id)
                .and_then(|parent| *parent)
                .is_none()
        })
        .map(|element| element.id)
        .collect();
    top_level.sort_by_key(|id| order.get(id).copied().unwrap_or(usize::MAX));

    let mut visible = Vec::new();
    let mut visited = HashSet::new();
    for top_id in top_level {
        push_outline_ids(
            top_id,
            &children_map,
            collapsed_outline_nodes,
            &mut visible,
            &mut visited,
        );
    }
    visible
}

fn format_date(rfc3339_date: &str) -> String {
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(rfc3339_date) {
        dt.format("%d.%m.%Y %H:%M").to_string()
    } else {
        rfc3339_date.to_string()
    }
}

fn default_bounds() -> CanvasBounds {
    CanvasBounds {
        position: CanvasPosition { x: 0.0, y: 0.0 },
        size: CanvasSize {
            width: 100.0,
            height: 100.0,
        },
    }
}

fn property_map(value: &serde_json::Value) -> Option<&serde_json::Map<String, serde_json::Value>> {
    value.as_object()
}

fn prop_str(
    props: Option<&serde_json::Map<String, serde_json::Value>>,
    key: &str,
    default: &str,
) -> String {
    props
        .and_then(|map| map.get(key))
        .and_then(|value| value.as_str())
        .unwrap_or(default)
        .to_string()
}

fn prop_f32(
    props: Option<&serde_json::Map<String, serde_json::Value>>,
    key: &str,
    default: f32,
) -> f32 {
    props
        .and_then(|map| map.get(key))
        .and_then(|value| value.as_f64())
        .map(|value| value as f32)
        .unwrap_or(default)
}

fn prop_bool(
    props: Option<&serde_json::Map<String, serde_json::Value>>,
    key: &str,
    default: bool,
) -> bool {
    props
        .and_then(|map| map.get(key))
        .and_then(|value| value.as_bool())
        .unwrap_or(default)
}

fn normalize_wrap_mode(value: &str) -> String {
    let lower = value.trim().to_ascii_lowercase();
    if lower == "nowrap" || lower == "no-wrap" {
        "nowrap".to_string()
    } else {
        "wrap".to_string()
    }
}

fn selected_container_mode(
    element_type: &str,
    props: Option<&serde_json::Map<String, serde_json::Value>>,
) -> String {
    match element_type {
        "stack-container" | "stack" => "stack".to_string(),
        "flex-container" | "flex" => "flex".to_string(),
        "grid-container" | "grid" => "grid".to_string(),
        "div" => prop_str(props, "container_mode", "absolute"),
        _ => "absolute".to_string(),
    }
}

fn parse_hex_color(value: &str) -> Option<Color> {
    if value.trim().eq_ignore_ascii_case("transparent") {
        return Some(Color::from_argb_u8(0, 0, 0, 0));
    }
    let hex = value.trim().strip_prefix('#')?;
    if !hex.is_ascii() || !hex.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return None;
    }
    let parse_pair = |s: &str| u8::from_str_radix(s, 16).ok();

    match hex.len() {
        3 => {
            let r = parse_pair(&hex[0..1].repeat(2))?;
            let g = parse_pair(&hex[1..2].repeat(2))?;
            let b = parse_pair(&hex[2..3].repeat(2))?;
            Some(Color::from_rgb_u8(r, g, b))
        }
        4 => {
            let r = parse_pair(&hex[0..1].repeat(2))?;
            let g = parse_pair(&hex[1..2].repeat(2))?;
            let b = parse_pair(&hex[2..3].repeat(2))?;
            let a = parse_pair(&hex[3..4].repeat(2))?;
            Some(Color::from_argb_u8(a, r, g, b))
        }
        6 => {
            let r = parse_pair(&hex[0..2])?;
            let g = parse_pair(&hex[2..4])?;
            let b = parse_pair(&hex[4..6])?;
            Some(Color::from_rgb_u8(r, g, b))
        }
        8 => {
            let r = parse_pair(&hex[0..2])?;
            let g = parse_pair(&hex[2..4])?;
            let b = parse_pair(&hex[4..6])?;
            let a = parse_pair(&hex[6..8])?;
            Some(Color::from_argb_u8(a, r, g, b))
        }
        _ => None,
    }
}

fn prop_color(
    props: Option<&serde_json::Map<String, serde_json::Value>>,
    key: &str,
    default: Color,
) -> Color {
    let raw = props
        .and_then(|map| map.get(key))
        .and_then(|value| value.as_str());
    raw.and_then(parse_hex_color).unwrap_or(default)
}

fn prop_image_path(
    props: Option<&serde_json::Map<String, serde_json::Value>>,
    key: &str,
) -> String {
    prop_str(props, key, "")
}

fn resolve_project_image_path(project: &Project, path: &str) -> Option<std::path::PathBuf> {
    let normalized = path.trim().replace('\\', "/");
    if normalized.is_empty() {
        return None;
    }

    if !normalized.starts_with("assets/") {
        return None;
    }

    let raw = Path::new(&normalized);
    if raw.is_absolute() {
        return None;
    }
    for component in raw.components() {
        if matches!(component, std::path::Component::ParentDir) {
            return None;
        }
    }

    Some(project.assets_root_dir().join(raw))
}

fn load_image_from_project_path(project: &Project, path: &str) -> Image {
    resolve_project_image_path(project, path)
        .and_then(|resolved| Image::load_from_path(&resolved).ok())
        .unwrap_or_default()
}

fn asset_display_name(path: &str) -> String {
    if path.trim().is_empty() {
        return String::new();
    }

    Path::new(path)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or(path)
        .to_string()
}

fn prop_uuid(
    props: Option<&serde_json::Map<String, serde_json::Value>>,
    key: &str,
) -> Option<Uuid> {
    props
        .and_then(|map| map.get(key))
        .and_then(|value| value.as_str())
        .and_then(|text| Uuid::parse_str(text).ok())
}

fn resolve_text_styles(
    element_map: &HashMap<Uuid, CanvasElementData>,
    parent_map: &HashMap<Uuid, Option<Uuid>>,
) -> HashMap<Uuid, ResolvedTextStyle> {
    let mut resolved = HashMap::new();
    let mut visiting = HashSet::new();

    for id in element_map.keys().copied() {
        let _ = resolve_text_style_for(id, element_map, parent_map, &mut resolved, &mut visiting);
    }

    resolved
}

fn resolve_text_style_for(
    element_id: Uuid,
    element_map: &HashMap<Uuid, CanvasElementData>,
    parent_map: &HashMap<Uuid, Option<Uuid>>,
    resolved: &mut HashMap<Uuid, ResolvedTextStyle>,
    visiting: &mut HashSet<Uuid>,
) -> ResolvedTextStyle {
    if let Some(existing) = resolved.get(&element_id) {
        return existing.clone();
    }

    if !visiting.insert(element_id) {
        return ResolvedTextStyle {
            font_size: 14.0,
            text_color: Color::from_rgb_u8(245, 245, 245),
            font_family: "Sans".to_string(),
            text_wrap: "wrap".to_string(),
        };
    }

    let parent_style = parent_map
        .get(&element_id)
        .and_then(|parent| *parent)
        .and_then(|parent_id| {
            element_map.get(&parent_id).map(|_| {
                resolve_text_style_for(parent_id, element_map, parent_map, resolved, visiting)
            })
        });

    let element = element_map.get(&element_id);
    let props = element.and_then(|value| property_map(&value.properties));
    let inherit_text_style = prop_bool(props, "inherit_text_style", true);

    let default_size = 14.0;
    let default_color = Color::from_rgb_u8(245, 245, 245);
    let default_family = "Sans".to_string();
    let default_wrap = "wrap".to_string();

    let own_style = ResolvedTextStyle {
        font_size: prop_f32(props, "font_size", default_size),
        text_color: prop_color(props, "text_color", default_color),
        font_family: prop_str(props, "font_family", &default_family),
        text_wrap: normalize_wrap_mode(&prop_str(props, "text_wrap", &default_wrap)),
    };

    let final_style = if inherit_text_style {
        parent_style.unwrap_or(own_style)
    } else {
        own_style
    };

    visiting.remove(&element_id);
    resolved.insert(element_id, final_style.clone());
    final_style
}

fn to_selection_info(
    project: &Project,
    element: &CanvasElementData,
    selected_ids: &HashSet<Uuid>,
    group_anchor: Option<Uuid>,
    depth: i32,
    has_children: bool,
    is_outline_expanded: bool,
    resolved_text_style: Option<&ResolvedTextStyle>,
) -> SelectionInfo {
    let props = property_map(&element.properties);
    let is_selected = selected_ids.contains(&element.id);
    let element_name = prop_str(
        props,
        "name",
        &prop_str(props, "display_name", &element.element_type.to_string()),
    );
    let resolved_font_size = resolved_text_style
        .map(|style| style.font_size)
        .unwrap_or(14.0);
    let resolved_text_color = resolved_text_style
        .map(|style| style.text_color)
        .unwrap_or_else(|| Color::from_rgb_u8(245, 245, 245));
    let resolved_font_family = resolved_text_style
        .map(|style| style.font_family.clone())
        .unwrap_or_else(|| "Sans".to_string());
    let resolved_text_wrap = resolved_text_style
        .map(|style| style.text_wrap.clone())
        .unwrap_or_else(|| "wrap".to_string());
    let bg_image_path = prop_image_path(props, "background_image");
    let image_source_path = prop_image_path(props, "image_src");
    let text_content = prop_str(props, "text", "");

    SelectionInfo {
        element_id: SharedString::from(element.id.to_string()),
        element_name: SharedString::from(element_name),
        element_type: SharedString::from(element.element_type.clone()),
        parent_id: SharedString::from(prop_str(props, "parent_id", "")),
        depth,
        has_children,
        is_outline_expanded,
        bounds: CanvasBounds {
            position: CanvasPosition {
                x: element.x,
                y: element.y,
            },
            size: CanvasSize {
                width: element.width,
                height: element.height,
            },
        },
        rotation: element.rotation,
        flip_horizontal: prop_bool(props, "flip_horizontal", false),
        flip_vertical: prop_bool(props, "flip_vertical", false),
        text_content: SharedString::from(text_content),
        placeholder: SharedString::from(prop_str(props, "placeholder", "")),
        checked: prop_bool(props, "checked", false),
        container_mode: SharedString::from(selected_container_mode(
            element.element_type.as_str(),
            props,
        )),
        allow_absolute_children: prop_bool(props, "allow_absolute_children", false),
        layout_padding: prop_f32(props, "layout_padding", 8.0),
        layout_padding_left: prop_f32(
            props,
            "layout_padding_left",
            prop_f32(props, "layout_padding", 8.0),
        ),
        layout_padding_right: prop_f32(
            props,
            "layout_padding_right",
            prop_f32(props, "layout_padding", 8.0),
        ),
        layout_padding_top: prop_f32(
            props,
            "layout_padding_top",
            prop_f32(props, "layout_padding", 8.0),
        ),
        layout_padding_bottom: prop_f32(
            props,
            "layout_padding_bottom",
            prop_f32(props, "layout_padding", 8.0),
        ),
        layout_spacing: prop_f32(props, "layout_spacing", 8.0),
        layout_margin: prop_f32(props, "layout_margin", 0.0),
        layout_margin_left: prop_f32(
            props,
            "layout_margin_left",
            prop_f32(props, "layout_margin", 0.0),
        ),
        layout_margin_right: prop_f32(
            props,
            "layout_margin_right",
            prop_f32(props, "layout_margin", 0.0),
        ),
        layout_margin_top: prop_f32(
            props,
            "layout_margin_top",
            prop_f32(props, "layout_margin", 0.0),
        ),
        layout_margin_bottom: prop_f32(
            props,
            "layout_margin_bottom",
            prop_f32(props, "layout_margin", 0.0),
        ),
        layout_order: prop_f32(props, "layout_order", 0.0),
        stack_alignment: SharedString::from(prop_str(props, "stack_alignment", "stretch")),
        flex_direction: SharedString::from(prop_str(props, "flex_direction", "column")),
        flex_wrap: SharedString::from(prop_str(props, "flex_wrap", "nowrap")),
        justify_items: SharedString::from(prop_str(props, "justify_items", "stretch")),
        justify_content: SharedString::from(prop_str(props, "justify_content", "flex-start")),
        align_items: SharedString::from(prop_str(props, "align_items", "stretch")),
        align_content: SharedString::from(prop_str(props, "align_content", "stretch")),
        place_items: SharedString::from(prop_str(props, "place_items", "stretch stretch")),
        flex_flow: SharedString::from(prop_str(props, "flex_flow", "column nowrap")),
        grid_template_columns: SharedString::from({
            let explicit = prop_str(props, "grid_template_columns", "");
            if explicit.trim().is_empty() {
                prop_str(props, "grid_columns", "1fr 1fr")
            } else {
                explicit
            }
        }),
        grid_template_rows: SharedString::from({
            let explicit = prop_str(props, "grid_template_rows", "");
            if explicit.trim().is_empty() {
                prop_str(props, "grid_rows", "auto auto")
            } else {
                explicit
            }
        }),
        grid_template_areas: SharedString::from(prop_str(props, "grid_template_areas", "")),
        checkbox_box_side: SharedString::from(prop_str(props, "checkbox_box_side", "left")),
        checkbox_check_color: prop_color(
            props,
            "checkbox_check_color",
            Color::from_rgb_u8(245, 245, 245),
        ),
        checkbox_box_color: prop_color(props, "checkbox_box_color", Color::from_rgb_u8(21, 21, 21)),
        checkbox_box_border_color: prop_color(
            props,
            "checkbox_box_border_color",
            Color::from_rgb_u8(74, 74, 74),
        ),
        checkbox_box_border_width: prop_f32(props, "checkbox_box_border_width", 1.0),
        checkbox_space_between: prop_bool(props, "checkbox_space_between", false),
        background: prop_color(props, "background", Color::from_rgb_u8(255, 255, 255)),
        background_image_path: SharedString::from(bg_image_path.clone()),
        background_image: load_image_from_project_path(project, &bg_image_path),
        border_color: prop_color(props, "border_color", Color::from_rgb_u8(74, 74, 74)),
        border_width: prop_f32(props, "border_width", 0.0),
        border_radius: prop_f32(props, "border_radius", 6.0),
        font_size: resolved_font_size,
        font_family: SharedString::from(resolved_font_family),
        text_wrap: SharedString::from(resolved_text_wrap),
        inherit_text_style: prop_bool(props, "inherit_text_style", true),
        image_source_path: SharedString::from(image_source_path.clone()),
        image_source: load_image_from_project_path(project, &image_source_path),
        text_color: resolved_text_color,
        opacity: prop_f32(props, "opacity", 1.0).clamp(0.0, 1.0),
        display_mode: SharedString::from(prop_str(props, "display_mode", "visible")),
        is_selected,
        is_group_anchor: group_anchor.map(|id| id == element.id).unwrap_or(false),
        is_hovered: false,
    }
}

fn set_selected_defaults(ui: &AppWindow) {
    ui.set_selected_element_id(SharedString::from(""));
    ui.set_selected_element_name(SharedString::from(""));
    ui.set_selected_element_type(SharedString::from(""));
    ui.set_selected_element_bounds(default_bounds());
    ui.set_selected_element_rotation(0.0);
    ui.set_selected_element_text(SharedString::from(""));
    ui.set_selected_element_checked(false);
    ui.set_selected_element_container_mode(SharedString::from("absolute"));
    ui.set_selected_element_allow_absolute_children(false);
    ui.set_selected_element_layout_padding(8.0);
    ui.set_selected_element_layout_padding_left(8.0);
    ui.set_selected_element_layout_padding_right(8.0);
    ui.set_selected_element_layout_padding_top(8.0);
    ui.set_selected_element_layout_padding_bottom(8.0);
    ui.set_selected_element_layout_spacing(8.0);
    ui.set_selected_element_layout_margin(0.0);
    ui.set_selected_element_layout_margin_left(0.0);
    ui.set_selected_element_layout_margin_right(0.0);
    ui.set_selected_element_layout_margin_top(0.0);
    ui.set_selected_element_layout_margin_bottom(0.0);
    ui.set_selected_element_layout_order(0.0);
    ui.set_selected_element_stack_alignment(SharedString::from("stretch"));
    ui.set_selected_element_flex_direction(SharedString::from("column"));
    ui.set_selected_element_flex_wrap(SharedString::from("nowrap"));
    ui.set_selected_element_justify_items(SharedString::from("stretch"));
    ui.set_selected_element_justify_content(SharedString::from("flex-start"));
    ui.set_selected_element_align_items(SharedString::from("stretch"));
    ui.set_selected_element_align_content(SharedString::from("stretch"));
    ui.set_selected_element_place_items(SharedString::from("stretch stretch"));
    ui.set_selected_element_flex_flow(SharedString::from("column nowrap"));
    ui.set_selected_element_grid_template_columns(SharedString::from("1fr 1fr"));
    ui.set_selected_element_grid_template_rows(SharedString::from("auto auto"));
    ui.set_selected_element_grid_template_areas(SharedString::from(""));
    ui.set_selected_element_checkbox_box_side(SharedString::from("left"));
    ui.set_selected_element_checkbox_check_color(SharedString::from("#f5f5f5"));
    ui.set_selected_element_checkbox_box_color(SharedString::from("#151515"));
    ui.set_selected_element_checkbox_box_border_color(SharedString::from("#4a4a4a"));
    ui.set_selected_element_checkbox_box_border_width(1.0);
    ui.set_selected_element_checkbox_space_between(false);
    ui.set_selected_element_background(SharedString::from("#ffffff"));
    ui.set_selected_element_border_color(SharedString::from("#4a4a4a"));
    ui.set_selected_element_border_width(0.0);
    ui.set_selected_element_border_radius(6.0);
    ui.set_selected_element_font_size(14.0);
    ui.set_selected_element_font_family(SharedString::from("Sans"));
    ui.set_selected_element_text_wrap(SharedString::from("wrap"));
    ui.set_selected_element_text_color(SharedString::from("#f5f5f5"));
    ui.set_selected_element_placeholder(SharedString::from(""));
    ui.set_selected_element_background_image(SharedString::from(""));
    ui.set_selected_element_image_source(SharedString::from(""));
    ui.set_selected_element_image_source_display(SharedString::from(""));
    ui.set_selected_element_opacity(1.0);
    ui.set_selected_element_display_mode(SharedString::from("visible"));
    ui.set_selected_element_inherit_text_style(true);
}

fn set_selected_element(
    ui: &AppWindow,
    _project: &Project,
    element: Option<&CanvasElementData>,
    resolved_text_style: Option<&ResolvedTextStyle>,
) {
    let Some(element) = element else {
        set_selected_defaults(ui);
        return;
    };

    let props = property_map(&element.properties);
    let selected_name = prop_str(
        props,
        "name",
        &prop_str(props, "display_name", &element.element_type.to_string()),
    );
    let resolved_font_size = resolved_text_style
        .map(|style| style.font_size)
        .unwrap_or(14.0);
    let resolved_text_color = resolved_text_style
        .map(|style| style.text_color)
        .map(|color| {
            format!(
                "#{:02x}{:02x}{:02x}",
                color.red(),
                color.green(),
                color.blue()
            )
        })
        .unwrap_or_else(|| "#f5f5f5".to_string());
    let resolved_font_family = resolved_text_style
        .map(|style| style.font_family.clone())
        .unwrap_or_else(|| "Sans".to_string());
    let resolved_text_wrap = resolved_text_style
        .map(|style| style.text_wrap.clone())
        .unwrap_or_else(|| "wrap".to_string());
    let selected_background = prop_str(props, "background", "#ffffff");
    let selected_background = if element.element_type == "text"
        || element.element_type == "label"
        || selected_background.eq_ignore_ascii_case("transparent")
    {
        "#0000".to_string()
    } else {
        selected_background
    };

    ui.set_selected_element_id(SharedString::from(element.id.to_string()));
    ui.set_selected_element_name(SharedString::from(selected_name));
    ui.set_selected_element_type(SharedString::from(element.element_type.clone()));
    ui.set_selected_element_bounds(CanvasBounds {
        position: CanvasPosition {
            x: element.x,
            y: element.y,
        },
        size: CanvasSize {
            width: element.width,
            height: element.height,
        },
    });
    ui.set_selected_element_rotation(element.rotation);
    ui.set_selected_element_text(SharedString::from(prop_str(props, "text", "")));
    ui.set_selected_element_checked(prop_bool(props, "checked", false));
    ui.set_selected_element_container_mode(SharedString::from(selected_container_mode(
        element.element_type.as_str(),
        props,
    )));
    ui.set_selected_element_allow_absolute_children(prop_bool(
        props,
        "allow_absolute_children",
        false,
    ));
    ui.set_selected_element_layout_padding(prop_f32(props, "layout_padding", 8.0));
    ui.set_selected_element_layout_padding_left(prop_f32(
        props,
        "layout_padding_left",
        prop_f32(props, "layout_padding", 8.0),
    ));
    ui.set_selected_element_layout_padding_right(prop_f32(
        props,
        "layout_padding_right",
        prop_f32(props, "layout_padding", 8.0),
    ));
    ui.set_selected_element_layout_padding_top(prop_f32(
        props,
        "layout_padding_top",
        prop_f32(props, "layout_padding", 8.0),
    ));
    ui.set_selected_element_layout_padding_bottom(prop_f32(
        props,
        "layout_padding_bottom",
        prop_f32(props, "layout_padding", 8.0),
    ));
    ui.set_selected_element_layout_spacing(prop_f32(props, "layout_spacing", 8.0));
    ui.set_selected_element_layout_margin(prop_f32(props, "layout_margin", 0.0));
    ui.set_selected_element_layout_margin_left(prop_f32(
        props,
        "layout_margin_left",
        prop_f32(props, "layout_margin", 0.0),
    ));
    ui.set_selected_element_layout_margin_right(prop_f32(
        props,
        "layout_margin_right",
        prop_f32(props, "layout_margin", 0.0),
    ));
    ui.set_selected_element_layout_margin_top(prop_f32(
        props,
        "layout_margin_top",
        prop_f32(props, "layout_margin", 0.0),
    ));
    ui.set_selected_element_layout_margin_bottom(prop_f32(
        props,
        "layout_margin_bottom",
        prop_f32(props, "layout_margin", 0.0),
    ));
    ui.set_selected_element_layout_order(prop_f32(props, "layout_order", 0.0));
    ui.set_selected_element_stack_alignment(SharedString::from(prop_str(
        props,
        "stack_alignment",
        "stretch",
    )));
    ui.set_selected_element_flex_direction(SharedString::from(prop_str(
        props,
        "flex_direction",
        "column",
    )));
    ui.set_selected_element_flex_wrap(SharedString::from(prop_str(props, "flex_wrap", "nowrap")));
    ui.set_selected_element_justify_items(SharedString::from(prop_str(
        props,
        "justify_items",
        "stretch",
    )));
    ui.set_selected_element_justify_content(SharedString::from(prop_str(
        props,
        "justify_content",
        "flex-start",
    )));
    ui.set_selected_element_align_items(SharedString::from(prop_str(
        props,
        "align_items",
        "stretch",
    )));
    ui.set_selected_element_align_content(SharedString::from(prop_str(
        props,
        "align_content",
        "stretch",
    )));
    ui.set_selected_element_place_items(SharedString::from(prop_str(
        props,
        "place_items",
        "stretch stretch",
    )));
    ui.set_selected_element_flex_flow(SharedString::from(prop_str(
        props,
        "flex_flow",
        "column nowrap",
    )));
    ui.set_selected_element_grid_template_columns(SharedString::from({
        let explicit = prop_str(props, "grid_template_columns", "");
        if explicit.trim().is_empty() {
            prop_str(props, "grid_columns", "1fr 1fr")
        } else {
            explicit
        }
    }));
    ui.set_selected_element_grid_template_rows(SharedString::from({
        let explicit = prop_str(props, "grid_template_rows", "");
        if explicit.trim().is_empty() {
            prop_str(props, "grid_rows", "auto auto")
        } else {
            explicit
        }
    }));
    ui.set_selected_element_grid_template_areas(SharedString::from(prop_str(
        props,
        "grid_template_areas",
        "",
    )));
    ui.set_selected_element_checkbox_box_side(SharedString::from(prop_str(
        props,
        "checkbox_box_side",
        "left",
    )));
    ui.set_selected_element_checkbox_check_color(SharedString::from(prop_str(
        props,
        "checkbox_check_color",
        "#f5f5f5",
    )));
    ui.set_selected_element_checkbox_box_color(SharedString::from(prop_str(
        props,
        "checkbox_box_color",
        "#151515",
    )));
    ui.set_selected_element_checkbox_box_border_color(SharedString::from(prop_str(
        props,
        "checkbox_box_border_color",
        "#4a4a4a",
    )));
    ui.set_selected_element_checkbox_box_border_width(prop_f32(
        props,
        "checkbox_box_border_width",
        1.0,
    ));
    ui.set_selected_element_checkbox_space_between(prop_bool(
        props,
        "checkbox_space_between",
        false,
    ));
    ui.set_selected_element_background(SharedString::from(selected_background));
    ui.set_selected_element_border_color(SharedString::from(prop_str(
        props,
        "border_color",
        "#4a4a4a",
    )));
    ui.set_selected_element_border_width(prop_f32(props, "border_width", 0.0));
    ui.set_selected_element_border_radius(prop_f32(props, "border_radius", 6.0));
    ui.set_selected_element_font_size(resolved_font_size);
    ui.set_selected_element_font_family(SharedString::from(resolved_font_family));
    ui.set_selected_element_text_wrap(SharedString::from(resolved_text_wrap));
    ui.set_selected_element_text_color(SharedString::from(resolved_text_color));
    ui.set_selected_element_placeholder(SharedString::from(prop_str(props, "placeholder", "")));
    ui.set_selected_element_background_image(SharedString::from(prop_str(
        props,
        "background_image",
        "",
    )));
    let image_source_path = prop_str(props, "image_src", "");
    ui.set_selected_element_image_source(SharedString::from(image_source_path.clone()));
    ui.set_selected_element_image_source_display(SharedString::from(asset_display_name(
        &image_source_path,
    )));
    ui.set_selected_element_opacity(prop_f32(props, "opacity", 1.0).clamp(0.0, 1.0));
    ui.set_selected_element_display_mode(SharedString::from(prop_str(
        props,
        "display_mode",
        "visible",
    )));
    ui.set_selected_element_inherit_text_style(prop_bool(props, "inherit_text_style", true));
}

fn build_selection_state(
    selected_ids: &[Uuid],
    element_map: &HashMap<Uuid, CanvasElementData>,
) -> (Vec<Uuid>, HashSet<Uuid>, Option<Uuid>) {
    let mut seen_selected = HashSet::new();
    let selected_list: Vec<Uuid> = selected_ids
        .iter()
        .copied()
        .filter(|id| element_map.contains_key(id) && seen_selected.insert(*id))
        .collect();
    let selected_set: HashSet<Uuid> = selected_list.iter().copied().collect();
    let group_anchor = (selected_list.len() > 1).then(|| selected_list[0]);
    (selected_list, selected_set, group_anchor)
}

fn build_parent_map(elements: &[CanvasElementData]) -> HashMap<Uuid, Option<Uuid>> {
    let id_set: HashSet<Uuid> = elements.iter().map(|element| element.id).collect();
    let mut parents = HashMap::new();

    for element in elements {
        let props = property_map(&element.properties);
        let parent = prop_uuid(props, "parent_id")
            .filter(|parent_id| *parent_id != element.id && id_set.contains(parent_id));
        parents.insert(element.id, parent);
    }

    parents
}

fn build_children_map(
    elements: &[CanvasElementData],
    parents: &HashMap<Uuid, Option<Uuid>>,
) -> HashMap<Uuid, Vec<Uuid>> {
    let order: HashMap<Uuid, usize> = elements
        .iter()
        .enumerate()
        .map(|(idx, element)| (element.id, idx))
        .collect();
    let mut children: HashMap<Uuid, Vec<Uuid>> = HashMap::new();

    for element in elements {
        if let Some(parent_id) = parents.get(&element.id).and_then(|parent| *parent) {
            children.entry(parent_id).or_default().push(element.id);
        }
    }

    for ids in children.values_mut() {
        ids.sort_by_key(|id| order.get(id).copied().unwrap_or(usize::MAX));
    }

    children
}

fn push_outline(
    project: &Project,
    element_id: Uuid,
    depth: i32,
    element_map: &HashMap<Uuid, CanvasElementData>,
    children_map: &HashMap<Uuid, Vec<Uuid>>,
    collapsed_outline_nodes: &HashSet<Uuid>,
    selected_set: &HashSet<Uuid>,
    group_anchor: Option<Uuid>,
    resolved_text_styles: &HashMap<Uuid, ResolvedTextStyle>,
    out: &mut Vec<SelectionInfo>,
    visited: &mut HashSet<Uuid>,
) {
    if !visited.insert(element_id) {
        return;
    }
    let Some(element) = element_map.get(&element_id) else {
        return;
    };

    let has_children = children_map
        .get(&element_id)
        .map(|children| !children.is_empty())
        .unwrap_or(false);
    let is_outline_expanded = !collapsed_outline_nodes.contains(&element_id);
    out.push(to_selection_info(
        project,
        element,
        selected_set,
        group_anchor,
        depth,
        has_children,
        is_outline_expanded,
        resolved_text_styles.get(&element.id),
    ));

    if has_children && is_outline_expanded {
        if let Some(children) = children_map.get(&element_id) {
            for child_id in children {
                push_outline(
                    project,
                    *child_id,
                    depth + 1,
                    element_map,
                    children_map,
                    collapsed_outline_nodes,
                    selected_set,
                    group_anchor,
                    resolved_text_styles,
                    out,
                    visited,
                );
            }
        }
    }
}

fn push_outline_ids(
    element_id: Uuid,
    children_map: &HashMap<Uuid, Vec<Uuid>>,
    collapsed_outline_nodes: &HashSet<Uuid>,
    out: &mut Vec<Uuid>,
    visited: &mut HashSet<Uuid>,
) {
    if !visited.insert(element_id) {
        return;
    }

    out.push(element_id);

    let expanded = !collapsed_outline_nodes.contains(&element_id);
    if !expanded {
        return;
    }

    if let Some(children) = children_map.get(&element_id) {
        for child_id in children {
            push_outline_ids(
                *child_id,
                children_map,
                collapsed_outline_nodes,
                out,
                visited,
            );
        }
    }
}
