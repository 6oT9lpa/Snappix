use std::collections::{HashMap, HashSet};

use uuid::Uuid;

use crate::api::BlueprintProjectApi;
use crate::model::{
    BlueprintDocument, BlueprintDocumentKind, BlueprintFunctionSignature, BlueprintFunctionTarget,
    BlueprintGraph, BlueprintLink, BlueprintLocalVariable, BlueprintNode, BlueprintNodeKind,
    BlueprintOwner, BlueprintPinDirection, BlueprintPinType,
};
use crate::validate::{validate_project, BlueprintDiagnostic, BlueprintDiagnosticSeverity};

#[derive(Debug, Clone)]
pub struct BlueprintIrProject {
    pub documents: Vec<BlueprintIrDocument>,
}

#[derive(Debug, Clone)]
pub struct BlueprintIrDocument {
    pub id: Uuid,
    pub name: String,
    pub kind: BlueprintDocumentKind,
    pub owner: BlueprintOwner,
    pub functions: Vec<BlueprintIrFunction>,
}

#[derive(Debug, Clone)]
pub struct BlueprintIrFunction {
    pub graph_id: Uuid,
    pub source_node_id: Uuid,
    pub name: String,
    pub rust_name: String,
    pub trigger: BlueprintIrFunctionTrigger,
    pub signature: BlueprintFunctionSignature,
    pub statements: Vec<BlueprintIrStatement>,
}

#[derive(Debug, Clone)]
pub enum BlueprintIrFunctionTrigger {
    Event {
        element_id: Uuid,
        event_name: String,
    },
    Function,
}

#[derive(Debug, Clone)]
pub enum BlueprintIrStatement {
    SetElementText {
        node_id: Uuid,
        value_pin_id: Option<Uuid>,
        element_id: Uuid,
        page_id: Uuid,
        value: BlueprintIrValue,
    },
    SetVariable {
        node_id: Uuid,
        variable_id: Uuid,
        variable_name: String,
        value: BlueprintIrValue,
    },
    Branch {
        node_id: Uuid,
        condition_pin_id: Option<Uuid>,
        condition: BlueprintIrValue,
        true_statements: Vec<BlueprintIrStatement>,
        false_statements: Vec<BlueprintIrStatement>,
    },
    CallDocumentFunction {
        node_id: Uuid,
        target: BlueprintFunctionTarget,
        function_name: String,
        arguments: Vec<BlueprintIrValue>,
    },
    FunctionalNode {
        node_id: Uuid,
        functional_node_id: String,
        arguments: Vec<BlueprintIrValue>,
    },
    Return {
        node_id: Uuid,
        value: Option<BlueprintIrValue>,
    },
}

#[derive(Debug, Clone)]
pub enum BlueprintIrValue {
    StringLiteral {
        node_id: Uuid,
        pin_id: Uuid,
        value: String,
    },
    Parameter {
        node_id: Uuid,
        pin_id: Uuid,
        name: String,
    },
    Variable {
        node_id: Uuid,
        pin_id: Uuid,
        variable_id: Uuid,
        variable_name: String,
        data_type: BlueprintPinType,
    },
    Default(BlueprintPinType),
}

pub fn lower_project(
    documents: &[BlueprintDocument],
    api: &BlueprintProjectApi,
) -> Result<BlueprintIrProject, Vec<BlueprintDiagnostic>> {
    let diagnostics = validate_project(documents, api);
    if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == BlueprintDiagnosticSeverity::Error)
    {
        return Err(diagnostics);
    }

    let documents = documents.iter().map(lower_document).collect();
    Ok(BlueprintIrProject { documents })
}

fn lower_document(document: &BlueprintDocument) -> BlueprintIrDocument {
    let mut functions = Vec::new();
    for graph in &document.graphs {
        functions.extend(lower_graph(document, graph));
    }

    BlueprintIrDocument {
        id: document.id,
        name: document.name.clone(),
        kind: document.kind,
        owner: document.owner.clone(),
        functions,
    }
}

fn lower_graph(document: &BlueprintDocument, graph: &BlueprintGraph) -> Vec<BlueprintIrFunction> {
    let node_map: HashMap<Uuid, &BlueprintNode> =
        graph.nodes.iter().map(|node| (node.id, node)).collect();
    let variable_map: HashMap<Uuid, &BlueprintLocalVariable> = graph
        .local_variables
        .iter()
        .map(|variable| (variable.id, variable))
        .collect();
    let exec_links = build_exec_links(graph, &node_map);
    let data_links = build_data_links(graph);

    let mut functions = Vec::new();
    for entrypoint_id in &graph.entrypoints {
        let Some(node) = node_map.get(entrypoint_id).copied() else {
            continue;
        };
        if let BlueprintNodeKind::UiEvent {
            element_id,
            event_name,
        } = &node.kind
        {
            let start_pin_id = node
                .pins
                .iter()
                .find(|pin| {
                    pin.direction == BlueprintPinDirection::Output
                        && pin.data_type == BlueprintPinType::Exec
                })
                .map(|pin| pin.id);
            let signature = BlueprintFunctionSignature {
                name: format!("{}_{}", node.title.replace(' ', "_"), event_name),
                parameters: Vec::new(),
                return_type: BlueprintPinType::Void,
                is_public: false,
            };
            functions.push(BlueprintIrFunction {
                graph_id: graph.id,
                source_node_id: node.id,
                name: signature.name.clone(),
                rust_name: sanitize_ident(format!(
                    "{}_{}_{}",
                    document.name, element_id, event_name
                )),
                trigger: BlueprintIrFunctionTrigger::Event {
                    element_id: *element_id,
                    event_name: event_name.clone(),
                },
                signature,
                statements: follow_exec_chain(
                    document,
                    start_pin_id,
                    &node_map,
                    &variable_map,
                    &exec_links,
                    &data_links,
                    HashSet::new(),
                ),
            });
        }
    }

    for node in &graph.nodes {
        if let BlueprintNodeKind::FunctionEntry { signature } = &node.kind {
            let start_pin_id = node
                .pins
                .iter()
                .find(|pin| {
                    pin.direction == BlueprintPinDirection::Output
                        && pin.data_type == BlueprintPinType::Exec
                })
                .map(|pin| pin.id);
            functions.push(BlueprintIrFunction {
                graph_id: graph.id,
                source_node_id: node.id,
                name: signature.name.clone(),
                rust_name: sanitize_ident(format!("{}_{}", document.name, signature.name)),
                trigger: BlueprintIrFunctionTrigger::Function,
                signature: signature.clone(),
                statements: follow_exec_chain(
                    document,
                    start_pin_id,
                    &node_map,
                    &variable_map,
                    &exec_links,
                    &data_links,
                    HashSet::new(),
                ),
            });
        }
    }

    functions
}

fn build_exec_links<'a>(
    graph: &'a BlueprintGraph,
    node_map: &HashMap<Uuid, &'a BlueprintNode>,
) -> HashMap<Uuid, &'a BlueprintLink> {
    let exec_pin_ids: HashSet<Uuid> = node_map
        .values()
        .flat_map(|node| {
            node.pins
                .iter()
                .filter(|pin| pin.data_type == BlueprintPinType::Exec)
                .map(|pin| pin.id)
        })
        .collect();

    graph
        .links
        .iter()
        .filter(|link| exec_pin_ids.contains(&link.from_pin_id))
        .map(|link| (link.from_pin_id, link))
        .collect()
}

fn build_data_links(graph: &BlueprintGraph) -> HashMap<Uuid, &BlueprintLink> {
    graph
        .links
        .iter()
        .map(|link| (link.to_pin_id, link))
        .collect()
}

fn follow_exec_chain(
    document: &BlueprintDocument,
    start_exec_pin: Option<Uuid>,
    node_map: &HashMap<Uuid, &BlueprintNode>,
    variable_map: &HashMap<Uuid, &BlueprintLocalVariable>,
    exec_links: &HashMap<Uuid, &BlueprintLink>,
    data_links: &HashMap<Uuid, &BlueprintLink>,
    visited_nodes: HashSet<Uuid>,
) -> Vec<BlueprintIrStatement> {
    let mut statements = Vec::new();
    let mut current_exec_pin = start_exec_pin;
    let mut visited_nodes = visited_nodes;

    while let Some(exec_pin_id) = current_exec_pin {
        let Some(link) = exec_links.get(&exec_pin_id).copied() else {
            break;
        };
        let Some(node) = node_map.get(&link.to_node_id).copied() else {
            break;
        };
        if !visited_nodes.insert(node.id) {
            break;
        }

        match &node.kind {
            BlueprintNodeKind::SetElementText { element_id } => {
                let value_pin = node.pin_named("text");
                let value = value_pin
                    .map(|pin| resolve_value(node, pin.id, node_map, variable_map, data_links))
                    .unwrap_or(BlueprintIrValue::Default(BlueprintPinType::String));
                let value_pin_id = value_pin.map(|pin| pin.id);
                let page_id = match document.owner {
                    BlueprintOwner::Page { page_id } => page_id,
                    BlueprintOwner::Project => Uuid::nil(),
                };
                statements.push(BlueprintIrStatement::SetElementText {
                    node_id: node.id,
                    value_pin_id,
                    element_id: *element_id,
                    page_id,
                    value,
                });
            }
            BlueprintNodeKind::CallDocumentFunction { target, signature } => {
                let arguments = signature
                    .parameters
                    .iter()
                    .filter_map(|parameter| {
                        let pin = node.pin_named(parameter.name.as_str())?;
                        Some(resolve_value(
                            node,
                            pin.id,
                            node_map,
                            variable_map,
                            data_links,
                        ))
                    })
                    .collect();
                statements.push(BlueprintIrStatement::CallDocumentFunction {
                    node_id: node.id,
                    target: target.clone(),
                    function_name: signature.name.clone(),
                    arguments,
                });
            }
            BlueprintNodeKind::FunctionResult { return_type } => {
                let value = if *return_type == BlueprintPinType::Void {
                    None
                } else {
                    node.pin_named("result")
                        .map(|pin| resolve_value(node, pin.id, node_map, variable_map, data_links))
                };
                statements.push(BlueprintIrStatement::Return {
                    node_id: node.id,
                    value,
                });
            }
            BlueprintNodeKind::VariableSet { variable_id } => {
                if let Some(variable) = variable_map.get(variable_id).copied() {
                    let value_pin = node.pin_named("value");
                    let value = value_pin
                        .map(|pin| {
                            resolve_value(node, pin.id, node_map, variable_map, data_links)
                        })
                        .unwrap_or(BlueprintIrValue::Default(variable.data_type));
                    statements.push(BlueprintIrStatement::SetVariable {
                        node_id: node.id,
                        variable_id: variable.id,
                        variable_name: variable.name.clone(),
                        value,
                    });
                }
            }
            BlueprintNodeKind::Functional { node_id } => {
                if node_id == "if_statement" {
                    let condition_pin = node.pin_named("condition");
                    let condition = condition_pin
                        .map(|pin| {
                            resolve_value(node, pin.id, node_map, variable_map, data_links)
                        })
                        .unwrap_or(BlueprintIrValue::Default(BlueprintPinType::Bool));
                    let true_pin = node.pin_named("true").map(|pin| pin.id);
                    let false_pin = node.pin_named("false").map(|pin| pin.id);
                    statements.push(BlueprintIrStatement::Branch {
                        node_id: node.id,
                        condition_pin_id: condition_pin.map(|pin| pin.id),
                        condition,
                        true_statements: follow_exec_chain(
                            document,
                            true_pin,
                            node_map,
                            variable_map,
                            exec_links,
                            data_links,
                            visited_nodes.clone(),
                        ),
                        false_statements: follow_exec_chain(
                            document,
                            false_pin,
                            node_map,
                            variable_map,
                            exec_links,
                            data_links,
                            visited_nodes.clone(),
                        ),
                    });
                    break;
                }

                let arguments = node
                    .pins
                    .iter()
                    .filter(|pin| {
                        pin.direction == BlueprintPinDirection::Input
                            && pin.data_type != BlueprintPinType::Exec
                    })
                    .map(|pin| resolve_value(node, pin.id, node_map, variable_map, data_links))
                    .collect();
                statements.push(BlueprintIrStatement::FunctionalNode {
                    node_id: node.id,
                    functional_node_id: node_id.clone(),
                    arguments,
                });
            }
            _ => {}
        }

        current_exec_pin = node
            .pins
            .iter()
            .find(|pin| {
                pin.direction == BlueprintPinDirection::Output
                    && pin.data_type == BlueprintPinType::Exec
            })
            .map(|pin| pin.id);
    }

    statements
}

fn resolve_value(
    current_node: &BlueprintNode,
    target_pin_id: Uuid,
    node_map: &HashMap<Uuid, &BlueprintNode>,
    variable_map: &HashMap<Uuid, &BlueprintLocalVariable>,
    data_links: &HashMap<Uuid, &BlueprintLink>,
) -> BlueprintIrValue {
    let Some(link) = data_links.get(&target_pin_id).copied() else {
        let fallback_type = current_node
            .pins
            .iter()
            .find(|pin| pin.id == target_pin_id)
            .map(|pin| pin.data_type)
            .unwrap_or(BlueprintPinType::Void);
        return BlueprintIrValue::Default(fallback_type);
    };

    let Some(source_node) = node_map.get(&link.from_node_id).copied() else {
        return BlueprintIrValue::Default(BlueprintPinType::Void);
    };
    let source_pin_id = link.from_pin_id;

    match &source_node.kind {
        BlueprintNodeKind::LiteralString { value } => BlueprintIrValue::StringLiteral {
            node_id: source_node.id,
            pin_id: source_pin_id,
            value: value.clone(),
        },
        BlueprintNodeKind::FunctionEntry { signature } => {
            let parameter_name = source_node
                .pins
                .iter()
                .find(|pin| pin.id == source_pin_id)
                .map(|pin| pin.name.clone())
                .filter(|pin_name| {
                    signature
                        .parameters
                        .iter()
                        .any(|parameter| parameter.name == *pin_name)
                });
            if let Some(name) = parameter_name {
                BlueprintIrValue::Parameter {
                    node_id: source_node.id,
                    pin_id: source_pin_id,
                    name,
                }
            } else {
                BlueprintIrValue::Default(BlueprintPinType::Void)
            }
        }
        BlueprintNodeKind::VariableGet { variable_id } => {
            if let Some(variable) = variable_map.get(variable_id).copied() {
                BlueprintIrValue::Variable {
                    node_id: source_node.id,
                    pin_id: source_pin_id,
                    variable_id: variable.id,
                    variable_name: variable.name.clone(),
                    data_type: variable.data_type,
                }
            } else {
                BlueprintIrValue::Default(BlueprintPinType::Void)
            }
        }
        _ => {
            let data_type = source_node
                .pins
                .iter()
                .find(|pin| pin.id == source_pin_id)
                .map(|pin| pin.data_type)
                .unwrap_or(BlueprintPinType::Void);
            BlueprintIrValue::Default(data_type)
        }
    }
}

fn sanitize_ident(raw: impl Into<String>) -> String {
    let raw = raw.into();
    let mut ident = String::new();
    for character in raw.chars() {
        if character.is_ascii_alphanumeric() {
            ident.push(character.to_ascii_lowercase());
        } else {
            ident.push('_');
        }
    }
    if ident.is_empty() {
        "blueprint_fn".to_string()
    } else {
        ident
    }
}
