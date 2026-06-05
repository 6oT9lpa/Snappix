use core_blueprint::{
    builtin_node_descriptor, BlueprintDocumentKind, BlueprintGraph, BlueprintLink, BlueprintNode,
    BlueprintNodeContext, BlueprintNodeKind, BlueprintPinDirection, BlueprintPinKind,
    BlueprintPinType, BlueprintPoint,
};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ActionOutcome {
    pub changed: bool,
    pub affected_node_id: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlueprintCommand {
    AddCatalogNode {
        descriptor_id: String,
        position: BlueprintPoint,
    },
    BindCatalogEvent {
        node_id: Uuid,
        element_id: Uuid,
    },
    MoveNode {
        node_id: Uuid,
        position: BlueprintPoint,
    },
    DeleteNode {
        node_id: Uuid,
    },
    DuplicateNode {
        node_id: Uuid,
        position: BlueprintPoint,
    },
    ConnectExecNodesAt {
        source_node_id: Uuid,
        drop_position: BlueprintPoint,
    },
}

impl ActionOutcome {
    fn changed_node(node_id: Uuid) -> Self {
        Self {
            changed: true,
            affected_node_id: Some(node_id),
        }
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ProjectActionError {
    #[error("Blueprint node descriptor '{0}' is not registered")]
    DescriptorNotFound(String),
    #[error("Blueprint node descriptor '{descriptor_id}' is not valid in {document_kind:?}")]
    InvalidDescriptorContext {
        descriptor_id: String,
        document_kind: BlueprintDocumentKind,
    },
    #[error("Blueprint node '{0}' was not found")]
    NodeNotFound(Uuid),
    #[error("Blueprint node '{0}' is not a catalog event node")]
    NotCatalogEvent(Uuid),
    #[error(
        "Blueprint node descriptor '{descriptor_id}' cannot bind to source type '{source_type}'"
    )]
    InvalidEventSource {
        descriptor_id: String,
        source_type: String,
    },
    #[error("Blueprint node '{0}' does not expose an exec output pin")]
    MissingExecOutput(Uuid),
    #[error("Blueprint node '{0}' does not expose a data output pin")]
    MissingDataOutput(Uuid),
    #[error("Unknown source pin kind '{0}'")]
    InvalidSourcePinKind(String),
    #[error("No compatible target node was found at the drop point")]
    LinkTargetNotFound,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourcePinKind {
    Exec,
    Data,
}

impl SourcePinKind {
    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "exec" => Some(Self::Exec),
            "data" => Some(Self::Data),
            _ => None,
        }
    }
}

pub fn descriptor_allows_document(
    descriptor_context: BlueprintNodeContext,
    document_kind: BlueprintDocumentKind,
) -> bool {
    match descriptor_context {
        BlueprintNodeContext::Any => true,
        BlueprintNodeContext::Page => document_kind == BlueprintDocumentKind::PageBlueprint,
        BlueprintNodeContext::Server => document_kind == BlueprintDocumentKind::ServerBlueprint,
    }
}

pub fn add_catalog_node(
    graph: &mut BlueprintGraph,
    document_kind: BlueprintDocumentKind,
    descriptor_id: &str,
    preferred_position: BlueprintPoint,
) -> Result<ActionOutcome, ProjectActionError> {
    let descriptor = builtin_node_descriptor(descriptor_id)
        .ok_or_else(|| ProjectActionError::DescriptorNotFound(descriptor_id.to_string()))?;
    if !descriptor_allows_document(descriptor.context, document_kind) {
        return Err(ProjectActionError::InvalidDescriptorContext {
            descriptor_id: descriptor_id.to_string(),
            document_kind,
        });
    }

    let mut node = descriptor.instantiate(preferred_position);
    node.position = nearest_free_blueprint_position(
        &graph.nodes,
        node.position,
        blueprint_node_visual_size(&node),
    );
    let node_id = node.id;
    graph.nodes.push(node);
    if descriptor.category == "Events" && !is_bindable_ui_event_descriptor(&descriptor) {
        graph.entrypoints.push(node_id);
    }
    Ok(ActionOutcome::changed_node(node_id))
}

pub fn apply_blueprint_command(
    graph: &mut BlueprintGraph,
    document_kind: BlueprintDocumentKind,
    command: BlueprintCommand,
) -> Result<ActionOutcome, ProjectActionError> {
    match command {
        BlueprintCommand::AddCatalogNode {
            descriptor_id,
            position,
        } => add_catalog_node(graph, document_kind, &descriptor_id, position),
        BlueprintCommand::BindCatalogEvent {
            node_id,
            element_id,
        } => bind_catalog_event_node_to_element(graph, document_kind, node_id, element_id),
        BlueprintCommand::MoveNode { node_id, position } => move_node(graph, node_id, position),
        BlueprintCommand::DeleteNode { node_id } => delete_node(graph, node_id),
        BlueprintCommand::DuplicateNode { node_id, position } => {
            duplicate_node(graph, node_id, position)
        }
        BlueprintCommand::ConnectExecNodesAt {
            source_node_id,
            drop_position,
        } => connect_exec_nodes_at(graph, source_node_id, drop_position),
    }
}

pub fn bind_catalog_event_node_to_element(
    graph: &mut BlueprintGraph,
    document_kind: BlueprintDocumentKind,
    node_id: Uuid,
    element_id: Uuid,
) -> Result<ActionOutcome, ProjectActionError> {
    bind_catalog_event_node_to_element_with_source(graph, document_kind, node_id, element_id, None)
}

pub fn bind_catalog_event_node_to_element_type(
    graph: &mut BlueprintGraph,
    document_kind: BlueprintDocumentKind,
    node_id: Uuid,
    element_id: Uuid,
    element_type: &str,
) -> Result<ActionOutcome, ProjectActionError> {
    bind_catalog_event_node_to_element_with_source(
        graph,
        document_kind,
        node_id,
        element_id,
        Some(element_type),
    )
}

fn bind_catalog_event_node_to_element_with_source(
    graph: &mut BlueprintGraph,
    document_kind: BlueprintDocumentKind,
    node_id: Uuid,
    element_id: Uuid,
    element_type: Option<&str>,
) -> Result<ActionOutcome, ProjectActionError> {
    let node = graph
        .nodes
        .iter_mut()
        .find(|node| node.id == node_id)
        .ok_or(ProjectActionError::NodeNotFound(node_id))?;
    let BlueprintNodeKind::Catalog { descriptor_id } = &node.kind else {
        return Err(ProjectActionError::NotCatalogEvent(node_id));
    };
    let descriptor = builtin_node_descriptor(descriptor_id)
        .ok_or_else(|| ProjectActionError::DescriptorNotFound(descriptor_id.clone()))?;
    if !descriptor_allows_document(descriptor.context, document_kind) {
        return Err(ProjectActionError::InvalidDescriptorContext {
            descriptor_id: descriptor_id.clone(),
            document_kind,
        });
    }
    if descriptor.category != "Events" || !is_bindable_ui_event_descriptor(&descriptor) {
        return Err(ProjectActionError::NotCatalogEvent(node_id));
    }
    if let Some(element_type) = element_type {
        if !descriptor
            .tags
            .iter()
            .any(|tag| event_source_matches_descriptor_tag(element_type, tag))
        {
            return Err(ProjectActionError::InvalidEventSource {
                descriptor_id: descriptor.id,
                source_type: element_type.to_string(),
            });
        }
    }

    node.kind = BlueprintNodeKind::CatalogEvent {
        descriptor_id: descriptor.id,
        element_id,
    };
    if !graph.entrypoints.contains(&node_id) {
        graph.entrypoints.push(node_id);
    }
    Ok(ActionOutcome::changed_node(node_id))
}

pub fn move_node(
    graph: &mut BlueprintGraph,
    node_id: Uuid,
    position: BlueprintPoint,
) -> Result<ActionOutcome, ProjectActionError> {
    let node = graph
        .nodes
        .iter_mut()
        .find(|node| node.id == node_id)
        .ok_or(ProjectActionError::NodeNotFound(node_id))?;
    if node.position == position {
        return Ok(ActionOutcome {
            changed: false,
            affected_node_id: Some(node_id),
        });
    }
    node.position = position;
    Ok(ActionOutcome::changed_node(node_id))
}

pub fn delete_node(
    graph: &mut BlueprintGraph,
    node_id: Uuid,
) -> Result<ActionOutcome, ProjectActionError> {
    let position = graph
        .nodes
        .iter()
        .position(|node| node.id == node_id)
        .ok_or(ProjectActionError::NodeNotFound(node_id))?;
    graph.nodes.remove(position);
    graph
        .entrypoints
        .retain(|entrypoint| *entrypoint != node_id);
    graph
        .links
        .retain(|link| link.from_node_id != node_id && link.to_node_id != node_id);
    Ok(ActionOutcome::changed_node(node_id))
}

pub fn duplicate_node(
    graph: &mut BlueprintGraph,
    node_id: Uuid,
    preferred_position: BlueprintPoint,
) -> Result<ActionOutcome, ProjectActionError> {
    let source = graph
        .nodes
        .iter()
        .find(|node| node.id == node_id)
        .ok_or(ProjectActionError::NodeNotFound(node_id))?
        .clone();
    let mut duplicate = clone_blueprint_node_with_new_ids(&source);
    duplicate.position = nearest_free_blueprint_position(
        &graph.nodes,
        preferred_position,
        blueprint_node_visual_size(&duplicate),
    );
    let duplicate_id = duplicate.id;
    graph.nodes.push(duplicate);
    Ok(ActionOutcome::changed_node(duplicate_id))
}

pub fn connect_exec_nodes_at(
    graph: &mut BlueprintGraph,
    from_node_id: Uuid,
    drop_position: BlueprintPoint,
) -> Result<ActionOutcome, ProjectActionError> {
    connect_nodes_at(graph, from_node_id, SourcePinKind::Exec, 0, drop_position)
}

pub fn connect_nodes_at(
    graph: &mut BlueprintGraph,
    from_node_id: Uuid,
    source_pin_kind: SourcePinKind,
    source_pin_slot: usize,
    drop_position: BlueprintPoint,
) -> Result<ActionOutcome, ProjectActionError> {
    let source_node = graph
        .nodes
        .iter()
        .find(|node| node.id == from_node_id)
        .ok_or(ProjectActionError::NodeNotFound(from_node_id))?;
    let source_pins: Vec<_> = source_node
        .pins
        .iter()
        .filter(|pin| {
            pin.direction == BlueprintPinDirection::Output
                && match source_pin_kind {
                    SourcePinKind::Exec => pin.kind == BlueprintPinKind::Exec,
                    SourcePinKind::Data => pin.kind == BlueprintPinKind::Data,
                }
        })
        .collect();
    let source_pin = source_pins
        .get(source_pin_slot)
        .copied()
        .or_else(|| source_pins.first().copied())
        .ok_or(match source_pin_kind {
            SourcePinKind::Exec => ProjectActionError::MissingExecOutput(from_node_id),
            SourcePinKind::Data => ProjectActionError::MissingDataOutput(from_node_id),
        })?;
    let source_pin_id = source_pin.id;
    let source_data_type = source_pin.data_type;

    let target_node = graph
        .nodes
        .iter()
        .rev()
        .find(|node| {
            node.id != from_node_id
                && node.pins.iter().any(|pin| {
                    if pin.direction != BlueprintPinDirection::Input {
                        return false;
                    }
                    match source_pin_kind {
                        SourcePinKind::Exec => pin.kind == BlueprintPinKind::Exec,
                        SourcePinKind::Data => {
                            pin.kind == BlueprintPinKind::Data
                                && pin_types_are_compatible(source_data_type, pin.data_type)
                        }
                    }
                })
                && blueprint_node_contains_point(node, drop_position)
        })
        .ok_or(ProjectActionError::LinkTargetNotFound)?;
    let target_node_id = target_node.id;

    let target_pin_id = pick_target_input_pin_id(
        target_node,
        source_pin_kind,
        source_data_type,
        drop_position,
    )
    .ok_or(ProjectActionError::LinkTargetNotFound)?;

    if graph
        .links
        .iter()
        .any(|link| link.from_pin_id == source_pin_id && link.to_pin_id == target_pin_id)
    {
        return Ok(ActionOutcome {
            changed: false,
            affected_node_id: Some(from_node_id),
        });
    }

    match source_pin_kind {
        SourcePinKind::Exec => {
            graph.links.retain(|link| {
                link.from_pin_id != source_pin_id && link.to_pin_id != target_pin_id
            });
        }
        SourcePinKind::Data => {
            graph.links.retain(|link| link.to_pin_id != target_pin_id);
        }
    }
    graph.links.push(BlueprintLink::new(
        from_node_id,
        source_pin_id,
        target_node_id,
        target_pin_id,
    ));
    Ok(ActionOutcome::changed_node(from_node_id))
}

pub fn prune_incompatible_links(graph: &mut BlueprintGraph) -> usize {
    let before = graph.links.len();
    graph.links.retain(|link| {
        let Some(from_node) = graph.nodes.iter().find(|node| node.id == link.from_node_id) else {
            return false;
        };
        let Some(to_node) = graph.nodes.iter().find(|node| node.id == link.to_node_id) else {
            return false;
        };
        let Some(from_pin) = from_node.pins.iter().find(|pin| pin.id == link.from_pin_id) else {
            return false;
        };
        let Some(to_pin) = to_node.pins.iter().find(|pin| pin.id == link.to_pin_id) else {
            return false;
        };

        if from_pin.direction != BlueprintPinDirection::Output
            || to_pin.direction != BlueprintPinDirection::Input
            || from_pin.kind != to_pin.kind
        {
            return false;
        }

        match from_pin.kind {
            BlueprintPinKind::Exec => true,
            BlueprintPinKind::Data => {
                pin_types_are_compatible(from_pin.data_type, to_pin.data_type)
            }
        }
    });
    before.saturating_sub(graph.links.len())
}

fn pin_types_are_compatible(from: BlueprintPinType, to: BlueprintPinType) -> bool {
    from == to
        || from == BlueprintPinType::Any
        || to == BlueprintPinType::Any
        || (from == BlueprintPinType::Int && to == BlueprintPinType::Float)
        || (from == BlueprintPinType::Object
            && matches!(
                to,
                BlueprintPinType::UiElementRef
                    | BlueprintPinType::PageRef
                    | BlueprintPinType::ApiRef
            ))
        || (to == BlueprintPinType::Object
            && matches!(
                from,
                BlueprintPinType::UiElementRef
                    | BlueprintPinType::PageRef
                    | BlueprintPinType::ApiRef
            ))
}

fn event_source_matches_descriptor_tag(source_type: &str, tag: &str) -> bool {
    let source = source_type.to_lowercase();
    let descriptor_tag = tag.to_lowercase();
    source == descriptor_tag
        || source.starts_with(&(descriptor_tag.clone() + "-"))
        || source.ends_with(&("-".to_string() + &descriptor_tag))
        || source.contains(&(descriptor_tag.clone() + "_"))
}

pub fn blueprint_node_visual_size(node: &BlueprintNode) -> (i32, i32) {
    let is_event = match &node.kind {
        BlueprintNodeKind::UiEvent { .. } | BlueprintNodeKind::CatalogEvent { .. } => true,
        BlueprintNodeKind::Catalog { descriptor_id } => builtin_node_descriptor(descriptor_id)
            .map(|descriptor| descriptor.category == "Events")
            .unwrap_or(false),
        _ => false,
    };
    let exec_output_count = node
        .pins
        .iter()
        .filter(|pin| {
            pin.direction == BlueprintPinDirection::Output && pin.kind == BlueprintPinKind::Exec
        })
        .count();
    let data_input_count = node
        .pins
        .iter()
        .filter(|pin| {
            pin.direction == BlueprintPinDirection::Input && pin.kind == BlueprintPinKind::Data
        })
        .count();
    let data_output_count = node
        .pins
        .iter()
        .filter(|pin| {
            pin.direction == BlueprintPinDirection::Output && pin.kind == BlueprintPinKind::Data
        })
        .count();
    let is_variable_getter = matches!(node.kind, BlueprintNodeKind::VariableGet { .. });
    let is_compact_data_output_node = blueprint_node_uses_compact_data_output_size(node);
    let is_operator_node = blueprint_node_uses_operator_size(node);

    // This size model is consumed by placement, overlap detection and connect/drop
    // hit-testing. Keep it in sync with BlueprintCanvas.slint's rendered geometry.
    let height = if is_operator_node {
        78
    } else if is_variable_getter || is_compact_data_output_node {
        48
    } else if exec_output_count >= 3 {
        170
    } else if exec_output_count >= 2 && (data_input_count > 0 || data_output_count > 0) {
        154
    } else if exec_output_count >= 2 {
        126
    } else if data_input_count >= 2 || data_output_count >= 2 {
        142
    } else if data_input_count > 0 || data_output_count > 0 {
        118
    } else {
        94
    };
    let width = if is_event {
        250
    } else if is_operator_node {
        150
    } else if is_variable_getter || is_compact_data_output_node {
        160
    } else {
        270
    };
    (width, height)
}

fn blueprint_node_uses_operator_size(node: &BlueprintNode) -> bool {
    let descriptor_id = match &node.kind {
        BlueprintNodeKind::Catalog { descriptor_id }
        | BlueprintNodeKind::CatalogEvent { descriptor_id, .. } => descriptor_id.as_str(),
        BlueprintNodeKind::Functional { node_id } => node_id.as_str(),
        _ => return false,
    };

    descriptor_id.starts_with("math.")
        || descriptor_id.starts_with("compare.")
        || descriptor_id == "string.concat"
}

// Non-getter value/literal nodes with exactly one data output use getter-sized
// geometry, but the UI deliberately keeps their normal non-getter styling.
fn blueprint_node_uses_compact_data_output_size(node: &BlueprintNode) -> bool {
    if matches!(
        node.kind,
        BlueprintNodeKind::VariableGet { .. }
            | BlueprintNodeKind::UiEvent { .. }
            | BlueprintNodeKind::CatalogEvent { .. }
    ) {
        return false;
    }

    let has_exec_input = node.pins.iter().any(|pin| {
        pin.direction == BlueprintPinDirection::Input && pin.kind == BlueprintPinKind::Exec
    });
    let has_exec_output = node.pins.iter().any(|pin| {
        pin.direction == BlueprintPinDirection::Output && pin.kind == BlueprintPinKind::Exec
    });
    let has_data_input = node.pins.iter().any(|pin| {
        pin.direction == BlueprintPinDirection::Input && pin.kind == BlueprintPinKind::Data
    });
    let data_output_count = node
        .pins
        .iter()
        .filter(|pin| {
            pin.direction == BlueprintPinDirection::Output && pin.kind == BlueprintPinKind::Data
        })
        .count();

    !has_exec_input && !has_exec_output && !has_data_input && data_output_count == 1
}

fn clone_blueprint_node_with_new_ids(source: &BlueprintNode) -> BlueprintNode {
    let mut next = source.clone();
    next.id = Uuid::new_v4();
    for pin in &mut next.pins {
        pin.id = Uuid::new_v4();
    }
    next
}

fn is_bindable_ui_event_descriptor(descriptor: &core_blueprint::BlueprintNodeDescriptor) -> bool {
    descriptor.category == "Events" && descriptor.tags.iter().any(|tag| tag == "ui")
}

fn nearest_free_blueprint_position(
    nodes: &[BlueprintNode],
    preferred: BlueprintPoint,
    size: (i32, i32),
) -> BlueprintPoint {
    let mut candidate = preferred;
    let mut attempts = 0;
    while blueprint_position_overlaps(nodes, candidate, size) && attempts < 64 {
        candidate.x += 32;
        candidate.y += 32;
        attempts += 1;
    }
    candidate
}

fn blueprint_position_overlaps(
    nodes: &[BlueprintNode],
    candidate: BlueprintPoint,
    size: (i32, i32),
) -> bool {
    let left = candidate.x;
    let top = candidate.y;
    let right = candidate.x + size.0;
    let bottom = candidate.y + size.1;

    nodes.iter().any(|node| {
        let node_size = blueprint_node_visual_size(node);
        let node_left = node.position.x;
        let node_top = node.position.y;
        let node_right = node.position.x + node_size.0;
        let node_bottom = node.position.y + node_size.1;
        right > node_left && left < node_right && bottom > node_top && top < node_bottom
    })
}

fn blueprint_node_contains_point(node: &BlueprintNode, point: BlueprintPoint) -> bool {
    let (w, h) = blueprint_node_visual_size(node);
    let left = node.position.x;
    let top = node.position.y;
    let right = left + w;
    let bottom = top + h;
    point.x >= left && point.x <= right && point.y >= top && point.y <= bottom
}

fn pick_target_input_pin_id(
    node: &BlueprintNode,
    source_pin_kind: SourcePinKind,
    source_data_type: BlueprintPinType,
    drop_position: BlueprintPoint,
) -> Option<Uuid> {
    let compatible_inputs: Vec<_> = node
        .pins
        .iter()
        .filter(|pin| {
            if pin.direction != BlueprintPinDirection::Input {
                return false;
            }
            match source_pin_kind {
                SourcePinKind::Exec => pin.kind == BlueprintPinKind::Exec,
                SourcePinKind::Data => {
                    pin.kind == BlueprintPinKind::Data
                        && pin_types_are_compatible(source_data_type, pin.data_type)
                }
            }
        })
        .collect();

    if compatible_inputs.is_empty() {
        return None;
    }
    if compatible_inputs.len() == 1 {
        return compatible_inputs.first().map(|pin| pin.id);
    }

    compatible_inputs
        .into_iter()
        .min_by_key(|pin| {
            let anchor_y = node.position.y + node_pin_anchor_offset_y(node, pin.id);
            (anchor_y - drop_position.y).abs()
        })
        .map(|pin| pin.id)
}

fn node_pin_anchor_offset_y(node: &BlueprintNode, pin_id: Uuid) -> i32 {
    let Some(pin) = node.pins.iter().find(|pin| pin.id == pin_id) else {
        return 58;
    };
    let exec_output_count = node
        .pins
        .iter()
        .filter(|candidate| {
            candidate.direction == BlueprintPinDirection::Output
                && candidate.kind == BlueprintPinKind::Exec
        })
        .count();

    // Mirrors the visual anchor offsets in BlueprintCanvas.slint. This is used
    // when a wire is dropped onto a node so the nearest compatible input is stable.
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
                .unwrap_or(0) as i32;
            60 + index * 28
        }
        (BlueprintPinDirection::Input, BlueprintPinKind::Exec) => 58,
        (BlueprintPinDirection::Input, BlueprintPinKind::Data) => {
            let data_inputs: Vec<Uuid> = node
                .pins
                .iter()
                .filter(|candidate| {
                    candidate.direction == BlueprintPinDirection::Input
                        && candidate.kind == BlueprintPinKind::Data
                })
                .map(|candidate| candidate.id)
                .collect();
            let index = data_inputs
                .iter()
                .position(|candidate_id| *candidate_id == pin_id)
                .unwrap_or(0) as i32;
            let base = if matches!(node.kind, BlueprintNodeKind::VariableGet { .. })
                || blueprint_node_uses_compact_data_output_size(node)
            {
                24
            } else if blueprint_node_uses_operator_size(node) {
                26
            } else if exec_output_count >= 2 {
                112
            } else {
                88
            };
            base + index * 24
        }
        (BlueprintPinDirection::Output, BlueprintPinKind::Data) => {
            let data_outputs: Vec<Uuid> = node
                .pins
                .iter()
                .filter(|candidate| {
                    candidate.direction == BlueprintPinDirection::Output
                        && candidate.kind == BlueprintPinKind::Data
                })
                .map(|candidate| candidate.id)
                .collect();
            let index = data_outputs
                .iter()
                .position(|candidate_id| *candidate_id == pin_id)
                .unwrap_or(0) as i32;
            let base = if matches!(node.kind, BlueprintNodeKind::VariableGet { .. })
                || blueprint_node_uses_compact_data_output_size(node)
            {
                24
            } else if blueprint_node_uses_operator_size(node) {
                38
            } else if exec_output_count >= 2 {
                112
            } else {
                88
            };
            base + index * 24
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_blueprint::{BlueprintGraphKind, BlueprintLocalVariable, BlueprintPin};

    #[test]
    fn add_catalog_node_rejects_invalid_context() {
        let mut graph = BlueprintGraph::new("Events", BlueprintGraphKind::EventGraph);
        let err = add_catalog_node(
            &mut graph,
            BlueprintDocumentKind::PageBlueprint,
            "network.request",
            BlueprintPoint { x: 0, y: 0 },
        )
        .expect_err("server-only node should be rejected in page blueprint");

        assert!(matches!(
            err,
            ProjectActionError::InvalidDescriptorContext { .. }
        ));
        assert!(graph.nodes.is_empty());
    }

    #[test]
    fn bind_catalog_event_node_to_element_makes_entrypoint() {
        let mut graph = BlueprintGraph::new("Events", BlueprintGraphKind::EventGraph);
        let outcome = add_catalog_node(
            &mut graph,
            BlueprintDocumentKind::PageBlueprint,
            "event.button",
            BlueprintPoint { x: 10, y: 20 },
        )
        .expect("catalog event node");
        let node_id = outcome.affected_node_id.expect("node id");
        let element_id = Uuid::new_v4();

        let bind = bind_catalog_event_node_to_element(
            &mut graph,
            BlueprintDocumentKind::PageBlueprint,
            node_id,
            element_id,
        )
        .expect("bind event");

        assert!(bind.changed);
        assert!(graph.entrypoints.contains(&node_id));
        let node = graph.node_by_id(node_id).expect("bound node");
        assert!(matches!(
            &node.kind,
            BlueprintNodeKind::CatalogEvent {
                descriptor_id,
                element_id: bound_element_id
            } if descriptor_id == "event.button" && *bound_element_id == element_id
        ));
    }

    #[test]
    fn bind_catalog_event_rejects_non_event_catalog_node() {
        let mut graph = BlueprintGraph::new("Events", BlueprintGraphKind::EventGraph);
        let outcome = add_catalog_node(
            &mut graph,
            BlueprintDocumentKind::PageBlueprint,
            "flow.branch",
            BlueprintPoint { x: 10, y: 20 },
        )
        .expect("branch node");
        let node_id = outcome.affected_node_id.expect("node id");

        let err = bind_catalog_event_node_to_element(
            &mut graph,
            BlueprintDocumentKind::PageBlueprint,
            node_id,
            Uuid::new_v4(),
        )
        .expect_err("branch is not an event");

        assert_eq!(err, ProjectActionError::NotCatalogEvent(node_id));
        assert!(!graph.entrypoints.contains(&node_id));
    }

    #[test]
    fn bind_catalog_event_rejects_invalid_document_context() {
        let mut graph = BlueprintGraph::new("Events", BlueprintGraphKind::EventGraph);
        let node = builtin_node_descriptor("event.button")
            .expect("event descriptor")
            .instantiate(BlueprintPoint { x: 10, y: 20 });
        let node_id = node.id;
        graph.nodes.push(node);

        let err = bind_catalog_event_node_to_element(
            &mut graph,
            BlueprintDocumentKind::ServerBlueprint,
            node_id,
            Uuid::new_v4(),
        )
        .expect_err("page event should be rejected in server blueprint");

        assert!(matches!(
            err,
            ProjectActionError::InvalidDescriptorContext { .. }
        ));
        assert!(!graph.entrypoints.contains(&node_id));
    }

    #[test]
    fn bind_catalog_event_rejects_wrong_element_type() {
        let mut graph = BlueprintGraph::new("Events", BlueprintGraphKind::EventGraph);
        let outcome = add_catalog_node(
            &mut graph,
            BlueprintDocumentKind::PageBlueprint,
            "event.button",
            BlueprintPoint { x: 10, y: 20 },
        )
        .expect("catalog event node");
        let node_id = outcome.affected_node_id.expect("node id");

        let err = bind_catalog_event_node_to_element_type(
            &mut graph,
            BlueprintDocumentKind::PageBlueprint,
            node_id,
            Uuid::new_v4(),
            "input",
        )
        .expect_err("button event should not bind to input");

        assert!(matches!(err, ProjectActionError::InvalidEventSource { .. }));
        assert!(!graph.entrypoints.contains(&node_id));
    }

    #[test]
    fn add_global_catalog_event_becomes_entrypoint_immediately() {
        let mut graph = BlueprintGraph::new("Events", BlueprintGraphKind::EventGraph);
        let outcome = add_catalog_node(
            &mut graph,
            BlueprintDocumentKind::PageBlueprint,
            "event.app_started",
            BlueprintPoint { x: 0, y: 0 },
        )
        .expect("global event node");
        let node_id = outcome.affected_node_id.expect("node id");

        assert!(graph.entrypoints.contains(&node_id));
    }

    #[test]
    fn duplicate_node_generates_new_node_and_pin_ids_without_links() {
        let mut graph = BlueprintGraph::new("Events", BlueprintGraphKind::EventGraph);
        let mut node = BlueprintNode {
            id: Uuid::new_v4(),
            title: "Action".to_string(),
            kind: BlueprintNodeKind::Functional {
                node_id: "action".to_string(),
            },
            pins: vec![
                BlueprintPin::exec_input("in"),
                BlueprintPin::exec_output("then"),
            ],
            position: BlueprintPoint { x: 10, y: 20 },
        };
        let original_id = node.id;
        let original_pins: Vec<_> = node.pins.iter().map(|pin| pin.id).collect();
        graph.nodes.push(node.clone());

        let duplicate = duplicate_node(&mut graph, original_id, BlueprintPoint { x: 60, y: 70 })
            .expect("duplicate");
        let duplicate_id = duplicate.affected_node_id.expect("duplicate id");
        node = graph
            .node_by_id(duplicate_id)
            .expect("duplicate node")
            .clone();

        assert_ne!(node.id, original_id);
        for pin in &node.pins {
            assert!(!original_pins.contains(&pin.id));
        }
        assert!(graph.links.is_empty());
    }

    #[test]
    fn data_output_can_connect_to_multiple_inputs() {
        let mut graph = BlueprintGraph::new("Events", BlueprintGraphKind::EventGraph);
        let source = builtin_node_descriptor("value.bool_true")
            .expect("bool value")
            .instantiate(BlueprintPoint { x: 0, y: 0 });
        let source_id = source.id;
        let source_pin_id = source
            .pins
            .iter()
            .find(|pin| {
                pin.direction == BlueprintPinDirection::Output && pin.kind == BlueprintPinKind::Data
            })
            .expect("source output")
            .id;
        let branch_one = builtin_node_descriptor("flow.branch")
            .expect("branch")
            .instantiate(BlueprintPoint { x: 320, y: 0 });
        let branch_two = builtin_node_descriptor("flow.branch")
            .expect("branch")
            .instantiate(BlueprintPoint { x: 320, y: 180 });
        graph.nodes = vec![source, branch_one.clone(), branch_two.clone()];

        assert!(
            connect_nodes_at(
                &mut graph,
                source_id,
                SourcePinKind::Data,
                0,
                BlueprintPoint { x: 340, y: 90 },
            )
            .expect("first connection")
            .changed
        );
        assert!(
            connect_nodes_at(
                &mut graph,
                source_id,
                SourcePinKind::Data,
                0,
                BlueprintPoint { x: 340, y: 270 },
            )
            .expect("second connection")
            .changed
        );

        let outgoing: Vec<_> = graph
            .links
            .iter()
            .filter(|link| link.from_pin_id == source_pin_id)
            .collect();
        assert_eq!(outgoing.len(), 2);
        assert!(outgoing.iter().any(|link| link.to_node_id == branch_one.id));
        assert!(outgoing.iter().any(|link| link.to_node_id == branch_two.id));
    }

    #[test]
    fn object_getter_can_connect_to_set_opacity_element_input() {
        let mut graph = BlueprintGraph::new("Events", BlueprintGraphKind::EventGraph);
        let variable = BlueprintLocalVariable {
            id: Uuid::new_v4(),
            name: "TargetElement".to_string(),
            data_type: BlueprintPinType::Object,
            value: Some(serde_json::json!({
                "kind": "element",
                "id": Uuid::new_v4().to_string(),
                "name": "Button"
            })),
            item_type: None,
        };
        graph.local_variables.push(variable.clone());
        let getter = BlueprintNode::variable_get(&variable);
        let getter_id = getter.id;
        let set_opacity = builtin_node_descriptor("ui.set_opacity")
            .expect("set opacity")
            .instantiate(BlueprintPoint { x: 320, y: 0 });
        let set_opacity_id = set_opacity.id;
        graph.nodes = vec![getter, set_opacity];

        let outcome = connect_nodes_at(
            &mut graph,
            getter_id,
            SourcePinKind::Data,
            0,
            BlueprintPoint { x: 340, y: 88 },
        )
        .expect("object getter should connect to element input");

        assert!(outcome.changed);
        let link = graph.links.first().expect("link");
        assert_eq!(link.to_node_id, set_opacity_id);
        let target_pin = graph
            .nodes
            .iter()
            .find(|node| node.id == set_opacity_id)
            .and_then(|node| node.pins.iter().find(|pin| pin.id == link.to_pin_id))
            .expect("target pin");
        assert_eq!(target_pin.name, "element");
        assert_eq!(target_pin.data_type, BlueprintPinType::UiElementRef);
    }

    #[test]
    fn int_getter_can_connect_to_set_opacity_float_input() {
        let mut graph = BlueprintGraph::new("Events", BlueprintGraphKind::EventGraph);
        let variable = BlueprintLocalVariable {
            id: Uuid::new_v4(),
            name: "Opacity".to_string(),
            data_type: BlueprintPinType::Int,
            value: Some(serde_json::json!(1)),
            item_type: None,
        };
        graph.local_variables.push(variable.clone());
        let getter = BlueprintNode::variable_get(&variable);
        let getter_id = getter.id;
        let set_opacity = builtin_node_descriptor("ui.set_opacity")
            .expect("set opacity")
            .instantiate(BlueprintPoint { x: 320, y: 0 });
        let set_opacity_id = set_opacity.id;
        graph.nodes = vec![getter, set_opacity];

        let outcome = connect_nodes_at(
            &mut graph,
            getter_id,
            SourcePinKind::Data,
            0,
            BlueprintPoint { x: 340, y: 112 },
        )
        .expect("int getter should connect to float opacity input");

        assert!(outcome.changed);
        let link = graph.links.first().expect("link");
        assert_eq!(link.to_node_id, set_opacity_id);
        let target_pin = graph
            .nodes
            .iter()
            .find(|node| node.id == set_opacity_id)
            .and_then(|node| node.pins.iter().find(|pin| pin.id == link.to_pin_id))
            .expect("target pin");
        assert_eq!(target_pin.name, "opacity");
        assert_eq!(target_pin.data_type, BlueprintPinType::Float);
    }
}
