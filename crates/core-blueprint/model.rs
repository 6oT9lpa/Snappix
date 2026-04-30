use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlueprintDocumentKind {
    PageBlueprint,
    ServerBlueprint,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BlueprintOwner {
    Page { page_id: Uuid },
    Project,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogicData {
    #[serde(default)]
    pub documents: Vec<BlueprintDocument>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlueprintDocument {
    pub id: Uuid,
    pub name: String,
    pub kind: BlueprintDocumentKind,
    pub owner: BlueprintOwner,
    #[serde(default)]
    pub graphs: Vec<BlueprintGraph>,
    #[serde(default)]
    pub exports: Vec<BlueprintExport>,
}

impl BlueprintDocument {
    pub fn new_page(page_id: Uuid, page_name: &str) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: blueprint_name_for_page(page_name),
            kind: BlueprintDocumentKind::PageBlueprint,
            owner: BlueprintOwner::Page { page_id },
            graphs: vec![BlueprintGraph::new(
                "Events",
                BlueprintGraphKind::EventGraph,
            )],
            exports: Vec::new(),
        }
    }

    pub fn new_server() -> Self {
        Self {
            id: Uuid::new_v4(),
            name: "server.blp".to_string(),
            kind: BlueprintDocumentKind::ServerBlueprint,
            owner: BlueprintOwner::Project,
            graphs: vec![BlueprintGraph::new(
                "Server Events",
                BlueprintGraphKind::EventGraph,
            )],
            exports: Vec::new(),
        }
    }

    pub fn sync_exports(&mut self) {
        self.exports = self
            .graphs
            .iter()
            .filter_map(|graph| {
                let signature = graph.function_signature()?;
                signature.is_public.then_some(BlueprintExport {
                    graph_id: graph.id,
                    signature,
                })
            })
            .collect();
    }

    pub fn graph_by_id(&self, graph_id: Uuid) -> Option<&BlueprintGraph> {
        self.graphs.iter().find(|graph| graph.id == graph_id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlueprintGraphKind {
    EventGraph,
    FunctionGraph,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlueprintGraph {
    pub id: Uuid,
    pub name: String,
    pub graph_kind: BlueprintGraphKind,
    #[serde(default)]
    pub entrypoints: Vec<Uuid>,
    #[serde(default)]
    pub nodes: Vec<BlueprintNode>,
    #[serde(default)]
    pub links: Vec<BlueprintLink>,
    #[serde(default)]
    pub local_variables: Vec<BlueprintLocalVariable>,
}

impl BlueprintGraph {
    pub fn new(name: impl Into<String>, graph_kind: BlueprintGraphKind) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            graph_kind,
            entrypoints: Vec::new(),
            nodes: Vec::new(),
            links: Vec::new(),
            local_variables: Vec::new(),
        }
    }

    pub fn function_signature(&self) -> Option<BlueprintFunctionSignature> {
        self.nodes.iter().find_map(|node| match &node.kind {
            BlueprintNodeKind::FunctionEntry { signature } => Some(signature.clone()),
            _ => None,
        })
    }

    pub fn node_by_id(&self, node_id: Uuid) -> Option<&BlueprintNode> {
        self.nodes.iter().find(|node| node.id == node_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlueprintLocalVariable {
    pub id: Uuid,
    pub name: String,
    pub data_type: BlueprintPinType,
    #[serde(default)]
    pub item_type: Option<BlueprintPinType>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlueprintNode {
    pub id: Uuid,
    pub title: String,
    pub kind: BlueprintNodeKind,
    #[serde(default)]
    pub pins: Vec<BlueprintPin>,
    #[serde(default)]
    pub position: BlueprintPoint,
}

impl BlueprintNode {
    pub fn ui_event(element_id: Uuid, event_name: impl Into<String>) -> Self {
        let event_name = event_name.into();
        Self {
            id: Uuid::new_v4(),
            title: event_name.clone(),
            kind: BlueprintNodeKind::UiEvent {
                element_id,
                event_name,
            },
            pins: vec![BlueprintPin::exec_output("then")],
            position: BlueprintPoint::default(),
        }
    }

    pub fn set_element_text(element_id: Uuid) -> Self {
        Self {
            id: Uuid::new_v4(),
            title: "Set Text".to_string(),
            kind: BlueprintNodeKind::SetElementText { element_id },
            pins: vec![
                BlueprintPin::exec_input("in"),
                BlueprintPin::exec_output("then"),
                BlueprintPin::data_input("text", BlueprintPinType::String),
            ],
            position: BlueprintPoint::default(),
        }
    }

    pub fn literal_string(value: impl Into<String>) -> Self {
        let value = value.into();
        Self {
            id: Uuid::new_v4(),
            title: format!("\"{value}\""),
            kind: BlueprintNodeKind::LiteralString { value },
            pins: vec![BlueprintPin::data_output("value", BlueprintPinType::String)],
            position: BlueprintPoint::default(),
        }
    }

    pub fn variable_get(variable: &BlueprintLocalVariable) -> Self {
        Self {
            id: Uuid::new_v4(),
            title: variable.name.clone(),
            kind: BlueprintNodeKind::VariableGet {
                variable_id: variable.id,
            },
            pins: vec![BlueprintPin::data_output("value", variable.data_type)],
            position: BlueprintPoint::default(),
        }
    }

    pub fn variable_set(variable: &BlueprintLocalVariable) -> Self {
        Self {
            id: Uuid::new_v4(),
            title: format!("Set {}", variable.name),
            kind: BlueprintNodeKind::VariableSet {
                variable_id: variable.id,
            },
            pins: vec![
                BlueprintPin::exec_input("in"),
                BlueprintPin::data_input("value", variable.data_type),
                BlueprintPin::exec_output("then"),
            ],
            position: BlueprintPoint::default(),
        }
    }

    pub fn function_entry(signature: BlueprintFunctionSignature) -> Self {
        let mut pins = vec![BlueprintPin::exec_output("then")];
        pins.extend(signature.parameters.iter().map(|parameter| {
            BlueprintPin::data_output(parameter.name.clone(), parameter.data_type)
        }));
        Self {
            id: Uuid::new_v4(),
            title: format!("Function {}", signature.name),
            kind: BlueprintNodeKind::FunctionEntry { signature },
            pins,
            position: BlueprintPoint::default(),
        }
    }

    pub fn function_result(return_type: BlueprintPinType) -> Self {
        let mut pins = vec![BlueprintPin::exec_input("in")];
        if return_type != BlueprintPinType::Void {
            pins.push(BlueprintPin::data_input("result", return_type));
        }
        Self {
            id: Uuid::new_v4(),
            title: "Return".to_string(),
            kind: BlueprintNodeKind::FunctionResult { return_type },
            pins,
            position: BlueprintPoint::default(),
        }
    }

    pub fn call_document_function(
        target: BlueprintFunctionTarget,
        signature: BlueprintFunctionSignature,
    ) -> Self {
        let mut pins = vec![
            BlueprintPin::exec_input("in"),
            BlueprintPin::exec_output("then"),
        ];
        pins.extend(signature.parameters.iter().map(|parameter| {
            BlueprintPin::data_input(parameter.name.clone(), parameter.data_type)
        }));
        if signature.return_type != BlueprintPinType::Void {
            pins.push(BlueprintPin::data_output("result", signature.return_type));
        }
        Self {
            id: Uuid::new_v4(),
            title: format!("Call {}", signature.name),
            kind: BlueprintNodeKind::CallDocumentFunction { target, signature },
            pins,
            position: BlueprintPoint::default(),
        }
    }

    pub fn pin_named(&self, name: &str) -> Option<&BlueprintPin> {
        self.pins.iter().find(|pin| pin.name == name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BlueprintNodeKind {
    UiEvent {
        element_id: Uuid,
        event_name: String,
    },
    SetElementText {
        element_id: Uuid,
    },
    LiteralString {
        value: String,
    },
    VariableGet {
        variable_id: Uuid,
    },
    VariableSet {
        variable_id: Uuid,
    },
    FunctionEntry {
        signature: BlueprintFunctionSignature,
    },
    FunctionResult {
        return_type: BlueprintPinType,
    },
    CallDocumentFunction {
        target: BlueprintFunctionTarget,
        signature: BlueprintFunctionSignature,
    },
    Catalog {
        descriptor_id: String,
    },
    Functional {
        node_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlueprintPin {
    pub id: Uuid,
    pub name: String,
    pub direction: BlueprintPinDirection,
    pub kind: BlueprintPinKind,
    pub data_type: BlueprintPinType,
}

impl BlueprintPin {
    pub fn exec_input(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            direction: BlueprintPinDirection::Input,
            kind: BlueprintPinKind::Exec,
            data_type: BlueprintPinType::Exec,
        }
    }

    pub fn exec_output(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            direction: BlueprintPinDirection::Output,
            kind: BlueprintPinKind::Exec,
            data_type: BlueprintPinType::Exec,
        }
    }

    pub fn data_input(name: impl Into<String>, data_type: BlueprintPinType) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            direction: BlueprintPinDirection::Input,
            kind: BlueprintPinKind::Data,
            data_type,
        }
    }

    pub fn data_output(name: impl Into<String>, data_type: BlueprintPinType) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            direction: BlueprintPinDirection::Output,
            kind: BlueprintPinKind::Data,
            data_type,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlueprintPinDirection {
    Input,
    Output,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlueprintPinKind {
    Exec,
    Data,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlueprintPinType {
    Exec,
    Any,
    Bool,
    Int,
    Float,
    String,
    Color,
    Array,
    Vector,
    HashSet,
    HashMap,
    Object,
    UiElementRef,
    PageRef,
    ApiRef,
    Void,
}

impl BlueprintPinType {
    pub fn is_numeric(self) -> bool {
        matches!(self, Self::Int | Self::Float)
    }

    pub fn default_numeric_width(self) -> Option<BlueprintNumericWidth> {
        self.is_numeric()
            .then_some(BlueprintNumericPolicy::default().default_width)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlueprintNumericWidth {
    Bits16,
    Bits32,
    Bits64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlueprintNumericPromotion {
    PromoteOnOverflow,
    Fixed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlueprintNumericPolicy {
    pub default_width: BlueprintNumericWidth,
    pub promotion: BlueprintNumericPromotion,
}

impl Default for BlueprintNumericPolicy {
    fn default() -> Self {
        Self {
            default_width: BlueprintNumericWidth::Bits16,
            promotion: BlueprintNumericPromotion::PromoteOnOverflow,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlueprintLink {
    pub id: Uuid,
    pub from_node_id: Uuid,
    pub from_pin_id: Uuid,
    pub to_node_id: Uuid,
    pub to_pin_id: Uuid,
}

impl BlueprintLink {
    pub fn new(from_node_id: Uuid, from_pin_id: Uuid, to_node_id: Uuid, to_pin_id: Uuid) -> Self {
        Self {
            id: Uuid::new_v4(),
            from_node_id,
            from_pin_id,
            to_node_id,
            to_pin_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlueprintExport {
    pub graph_id: Uuid,
    pub signature: BlueprintFunctionSignature,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlueprintFunctionSignature {
    pub name: String,
    #[serde(default)]
    pub parameters: Vec<BlueprintFunctionParameter>,
    pub return_type: BlueprintPinType,
    #[serde(default)]
    pub is_public: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlueprintFunctionParameter {
    pub name: String,
    pub data_type: BlueprintPinType,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BlueprintFunctionTarget {
    ThisDocument,
    Server,
    Page { page_id: Uuid },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlueprintPoint {
    pub x: i32,
    pub y: i32,
}

pub fn blueprint_name_for_page(page_name: &str) -> String {
    let normalized = page_name.trim().replace(' ', "_").to_ascii_lowercase();
    let base = if normalized.is_empty() {
        "page".to_string()
    } else {
        normalized
    };
    format!("{base}.blp")
}
