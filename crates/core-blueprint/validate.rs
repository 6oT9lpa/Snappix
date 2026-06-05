use std::collections::{HashMap, HashSet};

use uuid::Uuid;

use crate::api::BlueprintProjectApi;
use crate::catalog::{builtin_node_descriptor, BlueprintNodeContext};
use crate::model::{
    BlueprintDocument, BlueprintDocumentKind, BlueprintFunctionSignature, BlueprintFunctionTarget,
    BlueprintGraph, BlueprintLink, BlueprintNode, BlueprintNodeKind, BlueprintOwner, BlueprintPin,
    BlueprintPinDirection, BlueprintPinKind, BlueprintPinType,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlueprintDiagnosticSeverity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlueprintDiagnostic {
    pub severity: BlueprintDiagnosticSeverity,
    pub code: String,
    pub message: String,
    pub document_id: Option<Uuid>,
    pub graph_id: Option<Uuid>,
    pub node_id: Option<Uuid>,
    pub pin_id: Option<Uuid>,
    pub file: Option<String>,
    pub line: Option<usize>,
    pub column: Option<usize>,
}

impl BlueprintDiagnostic {
    fn error(
        code: impl Into<String>,
        message: impl Into<String>,
        document_id: Option<Uuid>,
        graph_id: Option<Uuid>,
        node_id: Option<Uuid>,
        pin_id: Option<Uuid>,
    ) -> Self {
        Self {
            severity: BlueprintDiagnosticSeverity::Error,
            code: code.into(),
            message: message.into(),
            document_id,
            graph_id,
            node_id,
            pin_id,
            file: None,
            line: None,
            column: None,
        }
    }

    fn warning(
        code: impl Into<String>,
        message: impl Into<String>,
        document_id: Option<Uuid>,
        graph_id: Option<Uuid>,
        node_id: Option<Uuid>,
        pin_id: Option<Uuid>,
    ) -> Self {
        Self {
            severity: BlueprintDiagnosticSeverity::Warning,
            code: code.into(),
            message: message.into(),
            document_id,
            graph_id,
            node_id,
            pin_id,
            file: None,
            line: None,
            column: None,
        }
    }
}

pub fn validate_project(
    documents: &[BlueprintDocument],
    api: &BlueprintProjectApi,
) -> Vec<BlueprintDiagnostic> {
    let document_map: HashMap<Uuid, &BlueprintDocument> = documents
        .iter()
        .map(|document| (document.id, document))
        .collect();

    let mut diagnostics = Vec::new();
    for document in documents {
        diagnostics.extend(validate_document(document, documents, api, &document_map));
    }
    diagnostics
}

fn validate_document(
    document: &BlueprintDocument,
    documents: &[BlueprintDocument],
    api: &BlueprintProjectApi,
    document_map: &HashMap<Uuid, &BlueprintDocument>,
) -> Vec<BlueprintDiagnostic> {
    let page_owner = match document.owner {
        BlueprintOwner::Page { page_id } => Some(page_id),
        BlueprintOwner::Project => None,
    };

    let mut diagnostics = Vec::new();
    if document.kind == BlueprintDocumentKind::PageBlueprint
        && page_owner.and_then(|page_id| api.page(page_id)).is_none()
    {
        diagnostics.push(BlueprintDiagnostic::error(
            "page_owner_missing",
            "Page blueprint owner is missing from the project API index.",
            Some(document.id),
            None,
            None,
            None,
        ));
    }

    for graph in &document.graphs {
        diagnostics.extend(validate_graph(
            document,
            graph,
            documents,
            api,
            document_map,
            page_owner,
        ));
    }

    diagnostics
}

fn validate_graph(
    document: &BlueprintDocument,
    graph: &BlueprintGraph,
    documents: &[BlueprintDocument],
    api: &BlueprintProjectApi,
    document_map: &HashMap<Uuid, &BlueprintDocument>,
    page_owner: Option<Uuid>,
) -> Vec<BlueprintDiagnostic> {
    let node_map: HashMap<Uuid, &BlueprintNode> =
        graph.nodes.iter().map(|node| (node.id, node)).collect();
    let pin_map: HashMap<Uuid, (&BlueprintNode, &BlueprintPin)> = graph
        .nodes
        .iter()
        .flat_map(|node| node.pins.iter().map(move |pin| (pin.id, (node, pin))))
        .collect();

    let mut diagnostics = Vec::new();
    for entrypoint in &graph.entrypoints {
        if !node_map.contains_key(entrypoint) {
            diagnostics.push(BlueprintDiagnostic::error(
                "entrypoint_missing",
                "Graph entrypoint references an unknown node.",
                Some(document.id),
                Some(graph.id),
                Some(*entrypoint),
                None,
            ));
        }
    }

    for link in &graph.links {
        let Some((from_node, from_pin)) = pin_map.get(&link.from_pin_id).copied() else {
            diagnostics.push(BlueprintDiagnostic::error(
                "link_source_missing",
                "Link source pin is missing.",
                Some(document.id),
                Some(graph.id),
                Some(link.from_node_id),
                Some(link.from_pin_id),
            ));
            continue;
        };
        let Some((to_node, to_pin)) = pin_map.get(&link.to_pin_id).copied() else {
            diagnostics.push(BlueprintDiagnostic::error(
                "link_target_missing",
                "Link target pin is missing.",
                Some(document.id),
                Some(graph.id),
                Some(link.to_node_id),
                Some(link.to_pin_id),
            ));
            continue;
        };

        if from_node.id != link.from_node_id || to_node.id != link.to_node_id {
            diagnostics.push(BlueprintDiagnostic::error(
                "link_endpoint_mismatch",
                "Link node and pin endpoints are inconsistent.",
                Some(document.id),
                Some(graph.id),
                None,
                None,
            ));
        }

        if from_pin.direction != BlueprintPinDirection::Output
            || to_pin.direction != BlueprintPinDirection::Input
        {
            diagnostics.push(BlueprintDiagnostic::error(
                "link_direction_invalid",
                "Links must connect output pins to input pins.",
                Some(document.id),
                Some(graph.id),
                Some(link.to_node_id),
                Some(link.to_pin_id),
            ));
        }

        if from_pin.kind != to_pin.kind
            || !pin_types_are_compatible(from_pin.data_type, to_pin.data_type)
        {
            diagnostics.push(BlueprintDiagnostic::error(
                "link_type_mismatch",
                format!(
                    "Pin type mismatch: {:?}/{:?} -> {:?}/{:?}.",
                    from_pin.kind, from_pin.data_type, to_pin.kind, to_pin.data_type
                ),
                Some(document.id),
                Some(graph.id),
                Some(link.to_node_id),
                Some(link.to_pin_id),
            ));
        }
    }

    for node in &graph.nodes {
        diagnostics.extend(validate_node(
            document,
            graph,
            node,
            documents,
            api,
            document_map,
            page_owner,
        ));
    }

    diagnostics
}

fn validate_node(
    document: &BlueprintDocument,
    graph: &BlueprintGraph,
    node: &BlueprintNode,
    documents: &[BlueprintDocument],
    api: &BlueprintProjectApi,
    document_map: &HashMap<Uuid, &BlueprintDocument>,
    page_owner: Option<Uuid>,
) -> Vec<BlueprintDiagnostic> {
    let mut diagnostics = Vec::new();
    match &node.kind {
        BlueprintNodeKind::UiEvent {
            element_id,
            event_name,
        } => {
            if document.kind == BlueprintDocumentKind::ServerBlueprint {
                diagnostics.push(BlueprintDiagnostic::error(
                    "server_ui_access_forbidden",
                    "Server blueprints cannot reference page UI events directly.",
                    Some(document.id),
                    Some(graph.id),
                    Some(node.id),
                    None,
                ));
                return diagnostics;
            }

            let Some(page_id) = page_owner else {
                return diagnostics;
            };
            let Some(page_api) = api.page(page_id) else {
                return diagnostics;
            };
            let Some(element_api) = page_api.element(*element_id) else {
                diagnostics.push(BlueprintDiagnostic::error(
                    "ui_element_missing",
                    "Blueprint node references a missing page element.",
                    Some(document.id),
                    Some(graph.id),
                    Some(node.id),
                    None,
                ));
                return diagnostics;
            };
            if element_api.event(event_name).is_none() {
                diagnostics.push(BlueprintDiagnostic::error(
                    "ui_event_missing",
                    format!(
                        "Event '{}' is not available for element '{}'.",
                        event_name, element_api.display_name
                    ),
                    Some(document.id),
                    Some(graph.id),
                    Some(node.id),
                    None,
                ));
            }
        }
        BlueprintNodeKind::SetElementText { element_id } => {
            if document.kind == BlueprintDocumentKind::ServerBlueprint {
                diagnostics.push(BlueprintDiagnostic::error(
                    "server_ui_access_forbidden",
                    "Server blueprints cannot modify page UI directly.",
                    Some(document.id),
                    Some(graph.id),
                    Some(node.id),
                    None,
                ));
                return diagnostics;
            }

            let Some(page_id) = page_owner else {
                return diagnostics;
            };
            let Some(page_api) = api.page(page_id) else {
                return diagnostics;
            };
            let Some(element_api) = page_api.element(*element_id) else {
                diagnostics.push(BlueprintDiagnostic::error(
                    "ui_element_missing",
                    "Blueprint node references a missing page element.",
                    Some(document.id),
                    Some(graph.id),
                    Some(node.id),
                    None,
                ));
                return diagnostics;
            };
            if element_api.action("set_text").is_none() {
                diagnostics.push(BlueprintDiagnostic::error(
                    "ui_action_missing",
                    format!(
                        "Action 'set_text' is not available for element '{}'.",
                        element_api.display_name
                    ),
                    Some(document.id),
                    Some(graph.id),
                    Some(node.id),
                    None,
                ));
            }
        }
        BlueprintNodeKind::FunctionEntry { signature } => {
            if signature.name.trim().is_empty() {
                diagnostics.push(BlueprintDiagnostic::error(
                    "function_name_empty",
                    "Function entry requires a non-empty function name.",
                    Some(document.id),
                    Some(graph.id),
                    Some(node.id),
                    None,
                ));
            }
        }
        BlueprintNodeKind::VariableGet { variable_id } => {
            if graph
                .local_variables
                .iter()
                .all(|variable| variable.id != *variable_id)
            {
                diagnostics.push(BlueprintDiagnostic::error(
                    "variable_missing",
                    "Variable node references a missing local variable.",
                    Some(document.id),
                    Some(graph.id),
                    Some(node.id),
                    None,
                ));
            }
        }
        BlueprintNodeKind::VariableSet { variable_id } => {
            let Some(variable) = graph
                .local_variables
                .iter()
                .find(|variable| variable.id == *variable_id)
            else {
                diagnostics.push(BlueprintDiagnostic::error(
                    "variable_missing",
                    "Variable node references a missing local variable.",
                    Some(document.id),
                    Some(graph.id),
                    Some(node.id),
                    None,
                ));
                return diagnostics;
            };
            if variable.data_type.is_collection() {
                diagnostics.extend(validate_collection_variable_setter(document, graph, node));
            }
        }
        BlueprintNodeKind::FunctionResult { return_type } => {
            let expected_return = graph
                .function_signature()
                .map(|signature| signature.return_type)
                .unwrap_or(BlueprintPinType::Void);
            if *return_type != expected_return {
                diagnostics.push(BlueprintDiagnostic::error(
                    "function_return_mismatch",
                    "Return node type does not match the function signature.",
                    Some(document.id),
                    Some(graph.id),
                    Some(node.id),
                    None,
                ));
            }
        }
        BlueprintNodeKind::CallDocumentFunction { target, signature } => {
            if let Some(expected) = resolve_function_target(
                document,
                target,
                signature.name.as_str(),
                documents,
                api,
                document_map,
            ) {
                diagnostics.extend(compare_signatures(
                    document.id,
                    graph.id,
                    node.id,
                    signature,
                    &expected,
                ));
            } else {
                diagnostics.push(BlueprintDiagnostic::error(
                    "function_target_missing",
                    format!(
                        "Function '{}' is not available at the selected target.",
                        signature.name
                    ),
                    Some(document.id),
                    Some(graph.id),
                    Some(node.id),
                    None,
                ));
            }

            if matches!(target, BlueprintFunctionTarget::Page { .. })
                && document.kind == BlueprintDocumentKind::PageBlueprint
            {
                diagnostics.push(BlueprintDiagnostic::error(
                    "page_to_page_call_forbidden",
                    "Page blueprints may call only server exports directly.",
                    Some(document.id),
                    Some(graph.id),
                    Some(node.id),
                    None,
                ));
            }
        }
        BlueprintNodeKind::Catalog { descriptor_id } => {
            if let Some(descriptor) = builtin_node_descriptor(descriptor_id) {
                if !descriptor_context_matches_document(descriptor.context, document.kind) {
                    diagnostics.push(BlueprintDiagnostic::error(
                        "catalog_node_context_invalid",
                        format!(
                            "Catalog node '{descriptor_id}' is not valid in {:?}.",
                            document.kind
                        ),
                        Some(document.id),
                        Some(graph.id),
                        Some(node.id),
                        None,
                    ));
                }
                if descriptor_id == "ui.set_display_mode" {
                    diagnostics.extend(validate_mode_pin_value(
                        document,
                        graph,
                        node,
                        "mode",
                        &["visible", "none"],
                        "display_mode",
                    ));
                }
            } else {
                diagnostics.push(BlueprintDiagnostic::error(
                    "catalog_node_missing",
                    format!("Catalog node '{descriptor_id}' is not registered."),
                    Some(document.id),
                    Some(graph.id),
                    Some(node.id),
                    None,
                ));
            }
        }
        BlueprintNodeKind::CatalogEvent {
            descriptor_id,
            element_id,
        } => {
            let mut descriptor_is_event = false;
            if let Some(descriptor) = builtin_node_descriptor(descriptor_id) {
                if !descriptor_context_matches_document(descriptor.context, document.kind) {
                    diagnostics.push(BlueprintDiagnostic::error(
                        "catalog_node_context_invalid",
                        format!(
                            "Catalog node '{descriptor_id}' is not valid in {:?}.",
                            document.kind
                        ),
                        Some(document.id),
                        Some(graph.id),
                        Some(node.id),
                        None,
                    ));
                }
                if descriptor.category != "Events" {
                    diagnostics.push(BlueprintDiagnostic::error(
                        "catalog_event_invalid",
                        format!("Catalog node '{descriptor_id}' is not an event descriptor."),
                        Some(document.id),
                        Some(graph.id),
                        Some(node.id),
                        None,
                    ));
                } else {
                    descriptor_is_event = true;
                }
            } else {
                diagnostics.push(BlueprintDiagnostic::error(
                    "catalog_node_missing",
                    format!("Catalog node '{descriptor_id}' is not registered."),
                    Some(document.id),
                    Some(graph.id),
                    Some(node.id),
                    None,
                ));
            }

            if document.kind == BlueprintDocumentKind::ServerBlueprint {
                diagnostics.push(BlueprintDiagnostic::error(
                    "server_ui_access_forbidden",
                    "Server blueprints cannot reference page UI events directly.",
                    Some(document.id),
                    Some(graph.id),
                    Some(node.id),
                    None,
                ));
                return diagnostics;
            }

            let Some(page_id) = page_owner else {
                return diagnostics;
            };
            let Some(page_api) = api.page(page_id) else {
                return diagnostics;
            };
            let Some(element_api) = page_api.element(*element_id) else {
                diagnostics.push(BlueprintDiagnostic::error(
                    "ui_element_missing",
                    "Catalog event node references a missing page element.",
                    Some(document.id),
                    Some(graph.id),
                    Some(node.id),
                    None,
                ));
                return diagnostics;
            };
            if descriptor_is_event {
                for pin in node.pins.iter().filter(|pin| {
                    pin.direction == BlueprintPinDirection::Output
                        && pin.data_type == BlueprintPinType::Exec
                }) {
                    if element_api.event(&pin.name).is_none() {
                        diagnostics.push(BlueprintDiagnostic::error(
                            "ui_event_missing",
                            format!(
                                "Event '{}' is not available for element '{}'.",
                                pin.name, element_api.display_name
                            ),
                            Some(document.id),
                            Some(graph.id),
                            Some(node.id),
                            Some(pin.id),
                        ));
                    }
                }
            }
        }
        BlueprintNodeKind::Functional { node_id } => {
            diagnostics.push(BlueprintDiagnostic::error(
                "functional_node_unavailable",
                format!(
                    "Functional node '{}' is not available in the current blueprint implementation.",
                    node_id
                ),
                Some(document.id),
                Some(graph.id),
                Some(node.id),
                None,
            ));
        }
        BlueprintNodeKind::LiteralString { .. } => {}
    }

    diagnostics
}

fn validate_collection_variable_setter(
    document: &BlueprintDocument,
    graph: &BlueprintGraph,
    node: &BlueprintNode,
) -> Vec<BlueprintDiagnostic> {
    let mut diagnostics = Vec::new();
    let Some(mode_pin) = node.pin_named("mode") else {
        diagnostics.push(BlueprintDiagnostic::error(
            "collection_mode_pin_missing",
            "Collection setter requires a mode input pin.",
            Some(document.id),
            Some(graph.id),
            Some(node.id),
            None,
        ));
        return diagnostics;
    };
    if mode_pin.kind != BlueprintPinKind::Data || mode_pin.direction != BlueprintPinDirection::Input
    {
        diagnostics.push(BlueprintDiagnostic::error(
            "collection_mode_pin_invalid",
            "Collection setter mode pin must be an input data pin.",
            Some(document.id),
            Some(graph.id),
            Some(node.id),
            Some(mode_pin.id),
        ));
    }

    diagnostics.extend(validate_mode_pin_value(
        document,
        graph,
        node,
        "mode",
        &["push", "pop"],
        "collection",
    ));

    let mode_connected = graph.links.iter().any(|link| link.to_pin_id == mode_pin.id);
    let static_mode = resolve_mode_pin_string_input(graph, node, mode_pin)
        .unwrap_or_else(|| "push".to_string())
        .trim()
        .to_ascii_lowercase();
    if static_mode == "push" || (mode_connected && static_mode != "pop") {
        let Some(value_pin) = node.pin_named("value") else {
            diagnostics.push(BlueprintDiagnostic::error(
                "collection_value_pin_missing",
                "Collection setter push mode requires a value input pin.",
                Some(document.id),
                Some(graph.id),
                Some(node.id),
                None,
            ));
            return diagnostics;
        };
        let value_connected = graph
            .links
            .iter()
            .any(|link| link.to_pin_id == value_pin.id);
        if !value_connected && input_value_is_empty(value_pin) {
            diagnostics.push(BlueprintDiagnostic::error(
                "collection_push_value_empty",
                "Collection setter push mode requires a non-empty value or a connected value pin.",
                Some(document.id),
                Some(graph.id),
                Some(node.id),
                Some(value_pin.id),
            ));
        }
    }

    diagnostics
}

fn validate_mode_pin_value(
    document: &BlueprintDocument,
    graph: &BlueprintGraph,
    node: &BlueprintNode,
    pin_name: &str,
    allowed: &[&str],
    code_prefix: &str,
) -> Vec<BlueprintDiagnostic> {
    let mut diagnostics = Vec::new();
    let Some(pin) = node.pin_named(pin_name) else {
        return diagnostics;
    };
    let Some(mode) = resolve_mode_pin_string_input(graph, node, pin) else {
        return diagnostics;
    };
    let normalized = mode.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        diagnostics.push(BlueprintDiagnostic::warning(
            format!("{code_prefix}_mode_empty"),
            format!("Mode pin '{pin_name}' must not be empty."),
            Some(document.id),
            Some(graph.id),
            Some(node.id),
            Some(pin.id),
        ));
    } else if !allowed.iter().any(|allowed| *allowed == normalized) {
        diagnostics.push(BlueprintDiagnostic::warning(
            format!("{code_prefix}_mode_invalid"),
            format!(
                "Mode pin '{pin_name}' must be one of: {}.",
                allowed.join(", ")
            ),
            Some(document.id),
            Some(graph.id),
            Some(node.id),
            Some(pin.id),
        ));
    }
    diagnostics
}

fn resolve_mode_pin_string_input(
    graph: &BlueprintGraph,
    node: &BlueprintNode,
    target_pin: &BlueprintPin,
) -> Option<String> {
    let static_value = resolve_known_string_input(graph, node, target_pin);
    let Some(variable_id) = connected_variable_get_id(graph, target_pin) else {
        return static_value;
    };

    resolve_string_variable_assignment_before_node(graph, node.id, variable_id, &mut HashSet::new())
        .or(static_value)
}

fn resolve_known_string_input(
    graph: &BlueprintGraph,
    node: &BlueprintNode,
    target_pin: &BlueprintPin,
) -> Option<String> {
    let Some(link) = graph
        .links
        .iter()
        .find(|link| link.to_pin_id == target_pin.id)
    else {
        return target_pin
            .value
            .as_ref()
            .and_then(|value| value.as_str())
            .map(|value| value.to_string());
    };
    let source_node = graph
        .nodes
        .iter()
        .find(|node| node.id == link.from_node_id)?;
    match &source_node.kind {
        BlueprintNodeKind::VariableGet { variable_id } => graph
            .local_variables
            .iter()
            .find(|variable| variable.id == *variable_id)
            .and_then(|variable| variable.value.as_ref())
            .and_then(|value| value.as_str())
            .map(|value| value.to_string())
            .or_else(|| Some(String::new())),
        BlueprintNodeKind::LiteralString { value } => Some(value.clone()),
        BlueprintNodeKind::Catalog { descriptor_id } if descriptor_id == "value.string_empty" => {
            Some(String::new())
        }
        _ => node
            .pins
            .iter()
            .find(|pin| pin.id == target_pin.id)
            .and_then(|pin| pin.value.as_ref())
            .and_then(|value| value.as_str())
            .map(|value| value.to_string()),
    }
}

fn connected_variable_get_id(graph: &BlueprintGraph, target_pin: &BlueprintPin) -> Option<Uuid> {
    let link = graph
        .links
        .iter()
        .find(|link| link.to_pin_id == target_pin.id)?;
    let source_node = graph.nodes.iter().find(|node| node.id == link.from_node_id)?;
    match &source_node.kind {
        BlueprintNodeKind::VariableGet { variable_id } => Some(*variable_id),
        _ => None,
    }
}

fn resolve_string_variable_assignment_before_node(
    graph: &BlueprintGraph,
    node_id: Uuid,
    variable_id: Uuid,
    visited: &mut HashSet<Uuid>,
) -> Option<String> {
    if !visited.insert(node_id) {
        return None;
    }

    for link in graph
        .links
        .iter()
        .filter(|link| link.to_node_id == node_id && link_targets_exec_input(graph, link))
    {
        let Some(source_node) = graph.nodes.iter().find(|node| node.id == link.from_node_id) else {
            continue;
        };
        if !link_sources_exec_output(source_node, link) {
            continue;
        }

        if matches!(
            &source_node.kind,
            BlueprintNodeKind::VariableSet {
                variable_id: source_variable_id
            } if *source_variable_id == variable_id
        ) {
            if let Some(value_pin) = source_node.pin_named("value") {
                if let Some(value) = resolve_known_string_input(graph, source_node, value_pin) {
                    return Some(value);
                }
            }
        }

        if let Some(value) = resolve_string_variable_assignment_before_node(
            graph,
            source_node.id,
            variable_id,
            visited,
        ) {
            return Some(value);
        }
    }

    None
}

fn link_targets_exec_input(graph: &BlueprintGraph, link: &BlueprintLink) -> bool {
    graph
        .nodes
        .iter()
        .find(|node| node.id == link.to_node_id)
        .and_then(|node| node.pins.iter().find(|pin| pin.id == link.to_pin_id))
        .is_some_and(|pin| {
            pin.kind == BlueprintPinKind::Exec && pin.direction == BlueprintPinDirection::Input
        })
}

fn link_sources_exec_output(node: &BlueprintNode, link: &BlueprintLink) -> bool {
    node.pins
        .iter()
        .find(|pin| pin.id == link.from_pin_id)
        .is_some_and(|pin| {
            pin.kind == BlueprintPinKind::Exec && pin.direction == BlueprintPinDirection::Output
        })
}

fn input_value_is_empty(pin: &BlueprintPin) -> bool {
    match pin.value.as_ref() {
        None => true,
        Some(value) if value.is_null() => true,
        Some(value) if value.as_str().is_some_and(|text| text.trim().is_empty()) => true,
        _ => false,
    }
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

fn descriptor_context_matches_document(
    context: BlueprintNodeContext,
    kind: BlueprintDocumentKind,
) -> bool {
    match context {
        BlueprintNodeContext::Any => true,
        BlueprintNodeContext::Page => kind == BlueprintDocumentKind::PageBlueprint,
        BlueprintNodeContext::Server => kind == BlueprintDocumentKind::ServerBlueprint,
    }
}

fn resolve_function_target(
    document: &BlueprintDocument,
    target: &BlueprintFunctionTarget,
    function_name: &str,
    documents: &[BlueprintDocument],
    api: &BlueprintProjectApi,
    document_map: &HashMap<Uuid, &BlueprintDocument>,
) -> Option<BlueprintFunctionSignature> {
    match target {
        BlueprintFunctionTarget::ThisDocument => document
            .exports
            .iter()
            .find(|export| export.signature.name == function_name)
            .map(|export| export.signature.clone())
            .or_else(|| {
                document.graphs.iter().find_map(|graph| {
                    let signature = graph.function_signature()?;
                    (signature.name == function_name).then_some(signature)
                })
            }),
        BlueprintFunctionTarget::Server => api
            .server_export(function_name)
            .map(|export| export.signature.clone()),
        BlueprintFunctionTarget::Page { page_id } => api
            .page_export(*page_id, function_name)
            .map(|export| export.signature.clone())
            .or_else(|| {
                documents.iter().find_map(|candidate| {
                    let owner_matches = matches!(
                        candidate.owner,
                        BlueprintOwner::Page { page_id: owner_id } if owner_id == *page_id
                    );
                    if !owner_matches {
                        return None;
                    }
                    document_map.get(&candidate.id).and_then(|doc| {
                        doc.exports
                            .iter()
                            .find(|export| export.signature.name == function_name)
                            .map(|export| export.signature.clone())
                    })
                })
            }),
    }
}

fn compare_signatures(
    document_id: Uuid,
    graph_id: Uuid,
    node_id: Uuid,
    actual: &BlueprintFunctionSignature,
    expected: &BlueprintFunctionSignature,
) -> Vec<BlueprintDiagnostic> {
    let mut diagnostics = Vec::new();
    if actual.parameters.len() != expected.parameters.len() {
        diagnostics.push(BlueprintDiagnostic::error(
            "function_param_count_mismatch",
            format!(
                "Function '{}' expects {} params but the call node defines {}.",
                expected.name,
                expected.parameters.len(),
                actual.parameters.len()
            ),
            Some(document_id),
            Some(graph_id),
            Some(node_id),
            None,
        ));
        return diagnostics;
    }

    if actual.return_type != expected.return_type {
        diagnostics.push(BlueprintDiagnostic::error(
            "function_return_type_mismatch",
            format!(
                "Function '{}' return type mismatch: {:?} vs {:?}.",
                expected.name, expected.return_type, actual.return_type
            ),
            Some(document_id),
            Some(graph_id),
            Some(node_id),
            None,
        ));
    }

    for (actual_param, expected_param) in actual.parameters.iter().zip(expected.parameters.iter()) {
        if actual_param.data_type != expected_param.data_type {
            diagnostics.push(BlueprintDiagnostic::error(
                "function_param_type_mismatch",
                format!(
                    "Param '{}' for function '{}' has type {:?}, expected {:?}.",
                    actual_param.name,
                    expected.name,
                    actual_param.data_type,
                    expected_param.data_type
                ),
                Some(document_id),
                Some(graph_id),
                Some(node_id),
                None,
            ));
        }
    }

    diagnostics
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use crate::api::{
        BlueprintProjectApi, PageApiDescriptor, ServerApiDescriptor, UiElementApiDescriptor,
        UiEventDescriptor,
    };
    use crate::catalog::builtin_node_descriptor;
    use crate::model::{
        BlueprintDocument, BlueprintFunctionSignature, BlueprintGraph, BlueprintGraphKind,
        BlueprintLink, BlueprintLocalVariable, BlueprintNode, BlueprintNodeKind, BlueprintPinType,
        BlueprintPoint,
    };

    use super::{validate_project, BlueprintDiagnosticSeverity};

    #[test]
    fn reports_missing_ui_element_reference() {
        let page_id = Uuid::new_v4();
        let missing_button_id = Uuid::new_v4();
        let mut document = BlueprintDocument::new_page(page_id, "Main");
        let event = BlueprintNode::ui_event(missing_button_id, "clicked");
        document.graphs[0].entrypoints.push(event.id);
        document.graphs[0].nodes.push(event);

        let api = BlueprintProjectApi {
            pages: vec![PageApiDescriptor {
                page_id,
                page_name: "Main".to_string(),
                elements: Vec::new(),
                exported_functions: Vec::new(),
            }],
            server: ServerApiDescriptor::default(),
        };

        let diagnostics = validate_project(&[document], &api);
        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "ui_element_missing"));
    }

    #[test]
    fn rejects_server_ui_access() {
        let mut document = BlueprintDocument::new_server();
        document
            .graphs
            .first_mut()
            .expect("server graph")
            .nodes
            .push(BlueprintNode::set_element_text(Uuid::new_v4()));

        let diagnostics = validate_project(&[document], &BlueprintProjectApi::default());
        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "server_ui_access_forbidden"));
    }

    #[test]
    fn rejects_incompatible_pin_links() {
        let page_id = Uuid::new_v4();
        let signature = BlueprintFunctionSignature {
            name: "CheckFlag".to_string(),
            parameters: Vec::new(),
            return_type: BlueprintPinType::Bool,
            is_public: false,
        };
        let entry = BlueprintNode::function_entry(signature.clone());
        let result = BlueprintNode::function_result(BlueprintPinType::Bool);
        let literal = BlueprintNode::literal_string("oops");

        let mut graph = BlueprintGraph::new("CheckFlag", BlueprintGraphKind::FunctionGraph);
        graph.nodes = vec![entry, result.clone(), literal.clone()];
        graph.links.push(BlueprintLink::new(
            literal.id,
            literal.pin_named("value").expect("value pin").id,
            result.id,
            result.pin_named("result").expect("result pin").id,
        ));

        let document = BlueprintDocument {
            id: Uuid::new_v4(),
            name: "main.blp".to_string(),
            kind: crate::model::BlueprintDocumentKind::PageBlueprint,
            owner: crate::model::BlueprintOwner::Page { page_id },
            graphs: vec![graph],
            exports: Vec::new(),
        };

        let api = BlueprintProjectApi {
            pages: vec![PageApiDescriptor {
                page_id,
                page_name: "Main".to_string(),
                elements: Vec::new(),
                exported_functions: Vec::new(),
            }],
            server: ServerApiDescriptor::default(),
        };

        let diagnostics = validate_project(&[document], &api);
        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "link_type_mismatch"));
    }

    #[test]
    fn rejects_functional_nodes_without_catalog() {
        let page_id = Uuid::new_v4();
        let mut document = BlueprintDocument::new_page(page_id, "Main");
        document.graphs[0].nodes.push(BlueprintNode {
            id: Uuid::new_v4(),
            title: "Old Functional Node".to_string(),
            kind: BlueprintNodeKind::Functional {
                node_id: "old_node".to_string(),
            },
            pins: Vec::new(),
            position: BlueprintPoint::default(),
        });

        let api = BlueprintProjectApi {
            pages: vec![PageApiDescriptor {
                page_id,
                page_name: "Main".to_string(),
                elements: Vec::new(),
                exported_functions: Vec::new(),
            }],
            server: ServerApiDescriptor::default(),
        };

        let diagnostics = validate_project(&[document], &api);
        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "functional_node_unavailable"));
    }

    #[test]
    fn rejects_server_catalog_node_in_page_blueprint() {
        let page_id = Uuid::new_v4();
        let mut document = BlueprintDocument::new_page(page_id, "Main");
        let node = builtin_node_descriptor("network.request")
            .expect("network descriptor")
            .instantiate(BlueprintPoint::default());
        document.graphs[0].nodes.push(node);

        let api = BlueprintProjectApi {
            pages: vec![PageApiDescriptor {
                page_id,
                page_name: "Main".to_string(),
                elements: Vec::new(),
                exported_functions: Vec::new(),
            }],
            server: ServerApiDescriptor::default(),
        };

        let diagnostics = validate_project(&[document], &api);
        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "catalog_node_context_invalid"));
    }

    #[test]
    fn rejects_page_catalog_node_in_server_blueprint() {
        let mut document = BlueprintDocument::new_server();
        let node = builtin_node_descriptor("page.navigate")
            .expect("page descriptor")
            .instantiate(BlueprintPoint::default());
        document.graphs[0].nodes.push(node);

        let diagnostics = validate_project(&[document], &BlueprintProjectApi::default());
        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "catalog_node_context_invalid"));
    }

    #[test]
    fn allows_any_catalog_node_in_page_blueprint() {
        let page_id = Uuid::new_v4();
        let mut document = BlueprintDocument::new_page(page_id, "Main");
        let node = builtin_node_descriptor("flow.branch")
            .expect("flow descriptor")
            .instantiate(BlueprintPoint::default());
        document.graphs[0].nodes.push(node);

        let api = BlueprintProjectApi {
            pages: vec![PageApiDescriptor {
                page_id,
                page_name: "Main".to_string(),
                elements: Vec::new(),
                exported_functions: Vec::new(),
            }],
            server: ServerApiDescriptor::default(),
        };

        let diagnostics = validate_project(&[document], &api);
        assert!(!diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "catalog_node_context_invalid"));
    }

    #[test]
    fn rejects_invalid_static_display_mode() {
        let page_id = Uuid::new_v4();
        let mut document = BlueprintDocument::new_page(page_id, "Main");
        let mut node = builtin_node_descriptor("ui.set_display_mode")
            .expect("display descriptor")
            .instantiate(BlueprintPoint::default());
        node.pin_named_mut("mode").expect("mode pin").value = Some(serde_json::json!("collapsed"));
        document.graphs[0].nodes.push(node);

        let diagnostics = validate_project(&[document], &BlueprintProjectApi::default());
        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "display_mode_mode_invalid"));
    }

    #[test]
    fn rejects_empty_string_variable_connected_to_collection_mode() {
        let page_id = Uuid::new_v4();
        let mut document = BlueprintDocument::new_page(page_id, "Main");
        let variable = BlueprintLocalVariable {
            id: Uuid::new_v4(),
            name: "items".to_string(),
            data_type: BlueprintPinType::Array,
            item_type: Some(BlueprintPinType::Object),
            value: None,
        };
        let mode_variable = BlueprintLocalVariable {
            id: Uuid::new_v4(),
            name: "mode".to_string(),
            data_type: BlueprintPinType::String,
            item_type: None,
            value: None,
        };
        let setter = BlueprintNode::variable_set(&variable);
        let getter = BlueprintNode::variable_get(&mode_variable);
        let mode_pin_id = setter.pin_named("mode").expect("mode pin").id;
        let getter_pin_id = getter.pin_named("value").expect("getter pin").id;
        document.graphs[0].local_variables.push(variable);
        document.graphs[0].local_variables.push(mode_variable);
        document.graphs[0].links.push(BlueprintLink::new(
            getter.id,
            getter_pin_id,
            setter.id,
            mode_pin_id,
        ));
        document.graphs[0].nodes.push(getter);
        document.graphs[0].nodes.push(setter);

        let diagnostics = validate_project(&[document], &BlueprintProjectApi::default());
        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "collection_mode_empty"
                && diagnostic.severity == BlueprintDiagnosticSeverity::Warning));
    }

    #[test]
    fn allows_string_variable_mode_when_exec_setter_assigns_value_before_use() {
        let page_id = Uuid::new_v4();
        let mut document = BlueprintDocument::new_page(page_id, "Main");
        let collection_variable = BlueprintLocalVariable {
            id: Uuid::new_v4(),
            name: "items".to_string(),
            data_type: BlueprintPinType::Array,
            item_type: Some(BlueprintPinType::Object),
            value: None,
        };
        let mode_variable = BlueprintLocalVariable {
            id: Uuid::new_v4(),
            name: "mode".to_string(),
            data_type: BlueprintPinType::String,
            item_type: None,
            value: None,
        };

        let mut mode_setter = BlueprintNode::variable_set(&mode_variable);
        mode_setter.pin_named_mut("value").expect("value pin").value =
            Some(serde_json::json!("pop"));
        let mode_getter = BlueprintNode::variable_get(&mode_variable);
        let collection_setter = BlueprintNode::variable_set(&collection_variable);

        let mode_getter_pin_id = mode_getter.pin_named("value").expect("getter pin").id;
        let collection_mode_pin_id = collection_setter
            .pin_named("mode")
            .expect("collection mode pin")
            .id;
        let mode_setter_exec_out = mode_setter.pin_named("then").expect("exec out").id;
        let collection_exec_in = collection_setter.pin_named("in").expect("exec in").id;

        document.graphs[0].local_variables.push(collection_variable);
        document.graphs[0].local_variables.push(mode_variable);
        document.graphs[0].links.push(BlueprintLink::new(
            mode_getter.id,
            mode_getter_pin_id,
            collection_setter.id,
            collection_mode_pin_id,
        ));
        document.graphs[0].links.push(BlueprintLink::new(
            mode_setter.id,
            mode_setter_exec_out,
            collection_setter.id,
            collection_exec_in,
        ));
        document.graphs[0].nodes.push(mode_setter);
        document.graphs[0].nodes.push(mode_getter);
        document.graphs[0].nodes.push(collection_setter);

        let diagnostics = validate_project(&[document], &BlueprintProjectApi::default());
        assert!(!diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "collection_mode_empty"));
        assert!(!diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "collection_mode_invalid"));
    }

    #[test]
    fn rejects_catalog_event_when_element_does_not_expose_pin_event() {
        let page_id = Uuid::new_v4();
        let button_id = Uuid::new_v4();
        let mut document = BlueprintDocument::new_page(page_id, "Main");
        let mut node = builtin_node_descriptor("event.button")
            .expect("button event descriptor")
            .instantiate(BlueprintPoint::default());
        node.kind = BlueprintNodeKind::CatalogEvent {
            descriptor_id: "event.button".to_string(),
            element_id: button_id,
        };
        document.graphs[0].entrypoints.push(node.id);
        document.graphs[0].nodes.push(node);

        let api = BlueprintProjectApi {
            pages: vec![PageApiDescriptor {
                page_id,
                page_name: "Main".to_string(),
                elements: vec![UiElementApiDescriptor {
                    element_id: button_id,
                    display_name: "Submit".to_string(),
                    element_type: "button".to_string(),
                    events: vec![UiEventDescriptor {
                        name: "clicked".to_string(),
                        display_name: "Clicked".to_string(),
                    }],
                    actions: Vec::new(),
                }],
                exported_functions: Vec::new(),
            }],
            server: ServerApiDescriptor::default(),
        };

        let diagnostics = validate_project(&[document], &api);
        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "ui_event_missing"));
    }
}
