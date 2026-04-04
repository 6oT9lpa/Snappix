use std::collections::HashMap;

use uuid::Uuid;

use crate::api::BlueprintProjectApi;
use crate::model::{
    BlueprintDocument, BlueprintDocumentKind, BlueprintFunctionSignature, BlueprintFunctionTarget,
    BlueprintGraph, BlueprintNode, BlueprintNodeKind, BlueprintOwner, BlueprintPin,
    BlueprintPinDirection, BlueprintPinType,
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
}

pub fn validate_project(
    documents: &[BlueprintDocument],
    api: &BlueprintProjectApi,
) -> Vec<BlueprintDiagnostic> {
    let document_map: HashMap<Uuid, &BlueprintDocument> =
        documents.iter().map(|document| (document.id, document)).collect();

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

        if from_pin.kind != to_pin.kind || from_pin.data_type != to_pin.data_type {
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
            if let Some(expected) =
                resolve_function_target(document, target, signature.name.as_str(), documents, api, document_map)
            {
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
                    format!("Function '{}' is not available at the selected target.", signature.name),
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
        BlueprintNodeKind::LiteralString { .. } => {}
    }

    diagnostics
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
                    actual_param.name, expected.name, actual_param.data_type, expected_param.data_type
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

    use crate::api::{BlueprintProjectApi, PageApiDescriptor, ServerApiDescriptor};
    use crate::model::{
        BlueprintDocument, BlueprintFunctionSignature, BlueprintGraph, BlueprintGraphKind,
        BlueprintLink, BlueprintNode, BlueprintPinType,
    };

    use super::validate_project;

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
        assert!(diagnostics.iter().any(|diagnostic| diagnostic.code == "ui_element_missing"));
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
}
