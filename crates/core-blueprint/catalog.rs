use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::model::{
    BlueprintNode, BlueprintNodeKind, BlueprintPin, BlueprintPinDirection, BlueprintPinKind,
    BlueprintPinType, BlueprintPoint,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlueprintNodeContext {
    Page,
    Server,
    Any,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlueprintNodeDescriptor {
    pub id: String,
    pub title: String,
    pub category: String,
    pub context: BlueprintNodeContext,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub pins: Vec<BlueprintPinDescriptor>,
}

impl BlueprintNodeDescriptor {
    pub fn instantiate(&self, position: BlueprintPoint) -> BlueprintNode {
        BlueprintNode {
            id: Uuid::new_v4(),
            title: self.title.clone(),
            kind: BlueprintNodeKind::Catalog {
                descriptor_id: self.id.clone(),
            },
            pins: self
                .pins
                .iter()
                .map(BlueprintPinDescriptor::instantiate)
                .collect(),
            position,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlueprintPinDescriptor {
    pub name: String,
    pub direction: BlueprintPinDirection,
    pub kind: BlueprintPinKind,
    pub data_type: BlueprintPinType,
}

impl BlueprintPinDescriptor {
    pub fn exec_input(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            direction: BlueprintPinDirection::Input,
            kind: BlueprintPinKind::Exec,
            data_type: BlueprintPinType::Exec,
        }
    }

    pub fn exec_output(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            direction: BlueprintPinDirection::Output,
            kind: BlueprintPinKind::Exec,
            data_type: BlueprintPinType::Exec,
        }
    }

    pub fn data_input(name: impl Into<String>, data_type: BlueprintPinType) -> Self {
        Self {
            name: name.into(),
            direction: BlueprintPinDirection::Input,
            kind: BlueprintPinKind::Data,
            data_type,
        }
    }

    pub fn data_output(name: impl Into<String>, data_type: BlueprintPinType) -> Self {
        Self {
            name: name.into(),
            direction: BlueprintPinDirection::Output,
            kind: BlueprintPinKind::Data,
            data_type,
        }
    }

    fn instantiate(&self) -> BlueprintPin {
        BlueprintPin {
            id: Uuid::new_v4(),
            name: self.name.clone(),
            direction: self.direction,
            kind: self.kind,
            data_type: self.data_type,
        }
    }
}

pub fn builtin_node_catalog() -> Vec<BlueprintNodeDescriptor> {
    vec![
        event_node(
            "event.button",
            "Event Button",
            &["clicked", "hovered", "pressed"],
            &["button", "clicked", "hovered", "pressed", "ui"],
        ),
        event_node(
            "event.input",
            "Event Input",
            &["changed", "focused", "blurred"],
            &["input", "changed", "focused", "blurred", "ui"],
        ),
        event_node(
            "event.checkbox",
            "Event Checkbox",
            &["clicked", "changed"],
            &["checkbox", "clicked", "changed", "ui"],
        ),
        event_node(
            "event.label",
            "Event Label",
            &["clicked", "hovered"],
            &["label", "clicked", "hovered", "ui"],
        ),
        event_node(
            "event.fill",
            "Event Fill",
            &["filled"],
            &[
                "text",
                "textarea",
                "label",
                "div",
                "container",
                "image",
                "fill",
                "ui",
            ],
        ),
        event_node(
            "event.app_started",
            "Event App Started",
            &["start"],
            &["program", "start", "app"],
        ),
        event_node(
            "event.app_started_delayed",
            "Event App Started Delayed",
            &["start"],
            &["program", "start", "delay"],
        ),
        event_node(
            "event.page_changed",
            "Event Page Changed",
            &["changed"],
            &["page", "changed", "navigation"],
        ),
        BlueprintNodeDescriptor {
            id: "page.navigate".to_string(),
            title: "Navigate Page".to_string(),
            category: "Pages".to_string(),
            context: BlueprintNodeContext::Page,
            tags: vec![
                "page".to_string(),
                "navigate".to_string(),
                "open".to_string(),
            ],
            pins: vec![
                BlueprintPinDescriptor::exec_input("in"),
                BlueprintPinDescriptor::data_input("page", BlueprintPinType::Object),
                BlueprintPinDescriptor::exec_output("then"),
            ],
        },
        BlueprintNodeDescriptor {
            id: "ui.set_opacity".to_string(),
            title: "Set Opacity".to_string(),
            category: "UI".to_string(),
            context: BlueprintNodeContext::Page,
            tags: vec!["opacity".to_string(), "alpha".to_string(), "ui".to_string()],
            pins: vec![
                BlueprintPinDescriptor::exec_input("in"),
                BlueprintPinDescriptor::data_input("element", BlueprintPinType::UiElementRef),
                BlueprintPinDescriptor::data_input("opacity", BlueprintPinType::Float),
                BlueprintPinDescriptor::exec_output("then"),
            ],
        },
        BlueprintNodeDescriptor {
            id: "ui.set_display_mode".to_string(),
            title: "Set Display Mode".to_string(),
            category: "UI".to_string(),
            context: BlueprintNodeContext::Page,
            tags: vec![
                "display".to_string(),
                "none".to_string(),
                "mode".to_string(),
            ],
            pins: vec![
                BlueprintPinDescriptor::exec_input("in"),
                BlueprintPinDescriptor::data_input("element", BlueprintPinType::UiElementRef),
                BlueprintPinDescriptor::data_input("mode", BlueprintPinType::String),
                BlueprintPinDescriptor::exec_output("then"),
            ],
        },
        BlueprintNodeDescriptor {
            id: "flow.branch".to_string(),
            title: "Branch".to_string(),
            category: "Flow".to_string(),
            context: BlueprintNodeContext::Any,
            tags: vec![
                "if".to_string(),
                "branch".to_string(),
                "condition".to_string(),
            ],
            pins: vec![
                BlueprintPinDescriptor::exec_input("in"),
                BlueprintPinDescriptor::data_input("condition", BlueprintPinType::Bool),
                BlueprintPinDescriptor::exec_output("true"),
                BlueprintPinDescriptor::exec_output("false"),
            ],
        },
        BlueprintNodeDescriptor {
            id: "network.request".to_string(),
            title: "HTTP Request".to_string(),
            category: "Network".to_string(),
            context: BlueprintNodeContext::Server,
            tags: vec!["http".to_string(), "request".to_string(), "api".to_string()],
            pins: vec![
                BlueprintPinDescriptor::exec_input("in"),
                BlueprintPinDescriptor::data_input("url", BlueprintPinType::String),
                BlueprintPinDescriptor::exec_output("then"),
                BlueprintPinDescriptor::data_output("response", BlueprintPinType::String),
            ],
        },
        BlueprintNodeDescriptor {
            id: "db.query".to_string(),
            title: "Database Query".to_string(),
            category: "Database".to_string(),
            context: BlueprintNodeContext::Server,
            tags: vec![
                "db".to_string(),
                "database".to_string(),
                "query".to_string(),
            ],
            pins: vec![
                BlueprintPinDescriptor::exec_input("in"),
                BlueprintPinDescriptor::data_input("query", BlueprintPinType::String),
                BlueprintPinDescriptor::exec_output("then"),
                BlueprintPinDescriptor::data_output("rows", BlueprintPinType::Array),
            ],
        },
    ]
}

pub fn builtin_node_descriptor(descriptor_id: &str) -> Option<BlueprintNodeDescriptor> {
    builtin_node_catalog()
        .into_iter()
        .find(|descriptor| descriptor.id == descriptor_id)
}

pub fn is_builtin_node_descriptor(descriptor_id: &str) -> bool {
    builtin_node_descriptor(descriptor_id).is_some()
}

fn event_node(id: &str, title: &str, outputs: &[&str], tags: &[&str]) -> BlueprintNodeDescriptor {
    BlueprintNodeDescriptor {
        id: id.to_string(),
        title: title.to_string(),
        category: "Events".to_string(),
        context: BlueprintNodeContext::Page,
        tags: tags.iter().map(|tag| (*tag).to_string()).collect(),
        pins: outputs
            .iter()
            .map(|name| BlueprintPinDescriptor::exec_output(*name))
            .collect(),
    }
}
