use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::model::{
    default_input_pin_value, BlueprintNode, BlueprintNodeKind, BlueprintPin, BlueprintPinDirection,
    BlueprintPinKind, BlueprintPinType, BlueprintPoint,
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
        let mut node = BlueprintNode {
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
        };
        if self.id == "ui.set_display_mode" {
            if let Some(pin) = node.pin_named_mut("mode") {
                pin.value = Some(serde_json::json!("visible"));
            }
        }
        node
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
            value: if self.direction == BlueprintPinDirection::Input
                && self.kind == BlueprintPinKind::Data
            {
                default_input_pin_value(self.data_type)
            } else {
                None
            },
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
            id: "flow.sequence".to_string(),
            title: "Sequence".to_string(),
            category: "Flow".to_string(),
            context: BlueprintNodeContext::Any,
            tags: vec![
                "sequence".to_string(),
                "flow".to_string(),
                "order".to_string(),
            ],
            pins: vec![
                BlueprintPinDescriptor::exec_input("in"),
                BlueprintPinDescriptor::exec_output("first"),
                BlueprintPinDescriptor::exec_output("then"),
            ],
        },
        BlueprintNodeDescriptor {
            id: "flow.delay".to_string(),
            title: "Delay".to_string(),
            category: "Flow".to_string(),
            context: BlueprintNodeContext::Any,
            tags: vec!["delay".to_string(), "wait".to_string(), "timer".to_string()],
            pins: vec![
                BlueprintPinDescriptor::exec_input("in"),
                BlueprintPinDescriptor::data_input("duration", BlueprintPinType::Float),
                BlueprintPinDescriptor::data_input("unit", BlueprintPinType::String),
                BlueprintPinDescriptor::exec_output("then"),
            ],
        },
        BlueprintNodeDescriptor {
            id: "flow.loop".to_string(),
            title: "Loop".to_string(),
            category: "Flow".to_string(),
            context: BlueprintNodeContext::Any,
            tags: vec![
                "loop".to_string(),
                "repeat".to_string(),
                "iteration".to_string(),
            ],
            pins: vec![
                BlueprintPinDescriptor::exec_input("in"),
                BlueprintPinDescriptor::data_input("condition", BlueprintPinType::Bool),
                BlueprintPinDescriptor::exec_output("body"),
                BlueprintPinDescriptor::exec_output("exit"),
            ],
        },
        BlueprintNodeDescriptor {
            id: "flow.while".to_string(),
            title: "While".to_string(),
            category: "Flow".to_string(),
            context: BlueprintNodeContext::Any,
            tags: vec![
                "while".to_string(),
                "loop".to_string(),
                "condition".to_string(),
            ],
            pins: vec![
                BlueprintPinDescriptor::exec_input("in"),
                BlueprintPinDescriptor::data_input("condition", BlueprintPinType::Bool),
                BlueprintPinDescriptor::exec_output("body"),
                BlueprintPinDescriptor::exec_output("exit"),
            ],
        },
        BlueprintNodeDescriptor {
            id: "flow.for_range".to_string(),
            title: "For Range".to_string(),
            category: "Flow".to_string(),
            context: BlueprintNodeContext::Any,
            tags: vec![
                "for".to_string(),
                "loop".to_string(),
                "range".to_string(),
                "index".to_string(),
            ],
            pins: vec![
                BlueprintPinDescriptor::exec_input("in"),
                BlueprintPinDescriptor::data_input("from", BlueprintPinType::Int),
                BlueprintPinDescriptor::data_input("to", BlueprintPinType::Int),
                BlueprintPinDescriptor::data_output("index", BlueprintPinType::Int),
                BlueprintPinDescriptor::exec_output("body"),
                BlueprintPinDescriptor::exec_output("exit"),
            ],
        },
        BlueprintNodeDescriptor {
            id: "flow.for_each".to_string(),
            title: "For Each".to_string(),
            category: "Flow".to_string(),
            context: BlueprintNodeContext::Any,
            tags: vec![
                "for".to_string(),
                "foreach".to_string(),
                "loop".to_string(),
                "collection".to_string(),
            ],
            pins: vec![
                BlueprintPinDescriptor::exec_input("in"),
                BlueprintPinDescriptor::data_input("items", BlueprintPinType::Array),
                BlueprintPinDescriptor::data_output("item", BlueprintPinType::Any),
                BlueprintPinDescriptor::data_output("index", BlueprintPinType::Int),
                BlueprintPinDescriptor::exec_output("body"),
                BlueprintPinDescriptor::exec_output("exit"),
            ],
        },
        BlueprintNodeDescriptor {
            id: "flow.return".to_string(),
            title: "Return".to_string(),
            category: "Flow".to_string(),
            context: BlueprintNodeContext::Any,
            tags: vec!["return".to_string(), "exit".to_string(), "flow".to_string()],
            pins: vec![
                BlueprintPinDescriptor::exec_input("in"),
                BlueprintPinDescriptor::data_input("value", BlueprintPinType::Any),
            ],
        },
        data_literal_node(
            "value.bool_true",
            "Bool True",
            "Values",
            BlueprintPinType::Bool,
            &["literal", "bool", "true", "value"],
        ),
        data_literal_node(
            "value.bool_false",
            "Bool False",
            "Values",
            BlueprintPinType::Bool,
            &["literal", "bool", "false", "value"],
        ),
        data_literal_node(
            "value.int_zero",
            "Int 0",
            "Values",
            BlueprintPinType::Int,
            &["literal", "int", "zero", "value"],
        ),
        data_literal_node(
            "value.float_zero",
            "Float 0.0",
            "Values",
            BlueprintPinType::Float,
            &["literal", "float", "zero", "value"],
        ),
        data_literal_node(
            "value.string_empty",
            "String Empty",
            "Values",
            BlueprintPinType::String,
            &["literal", "string", "empty", "value"],
        ),
        BlueprintNodeDescriptor {
            id: "bool.not".to_string(),
            title: "Bool Not".to_string(),
            category: "Values".to_string(),
            context: BlueprintNodeContext::Any,
            tags: vec![
                "bool".to_string(),
                "not".to_string(),
                "logic".to_string(),
                "value".to_string(),
            ],
            pins: vec![
                BlueprintPinDescriptor::data_input("value", BlueprintPinType::Bool),
                BlueprintPinDescriptor::data_output("result", BlueprintPinType::Bool),
            ],
        },
        BlueprintNodeDescriptor {
            id: "math.negate".to_string(),
            title: "Math Negate".to_string(),
            category: "Values".to_string(),
            context: BlueprintNodeContext::Any,
            tags: vec![
                "math".to_string(),
                "number".to_string(),
                "negate".to_string(),
                "value".to_string(),
            ],
            pins: vec![
                BlueprintPinDescriptor::data_input("value", BlueprintPinType::Float),
                BlueprintPinDescriptor::data_output("result", BlueprintPinType::Float),
            ],
        },
        BlueprintNodeDescriptor {
            id: "string.length".to_string(),
            title: "String Length".to_string(),
            category: "Values".to_string(),
            context: BlueprintNodeContext::Any,
            tags: vec![
                "string".to_string(),
                "length".to_string(),
                "text".to_string(),
                "value".to_string(),
            ],
            pins: vec![
                BlueprintPinDescriptor::data_input("text", BlueprintPinType::String),
                BlueprintPinDescriptor::data_output("length", BlueprintPinType::Int),
            ],
        },
        BlueprintNodeDescriptor {
            id: "string.is_empty".to_string(),
            title: "String Is Empty".to_string(),
            category: "Values".to_string(),
            context: BlueprintNodeContext::Any,
            tags: vec![
                "string".to_string(),
                "empty".to_string(),
                "text".to_string(),
                "value".to_string(),
            ],
            pins: vec![
                BlueprintPinDescriptor::data_input("text", BlueprintPinType::String),
                BlueprintPinDescriptor::data_output("result", BlueprintPinType::Bool),
            ],
        },
        math_value_node(
            "math.int.add",
            "Int Add",
            BlueprintPinType::Int,
            BlueprintPinType::Int,
            BlueprintPinType::Int,
            &["math", "int", "+", "add"],
        ),
        math_value_node(
            "math.int.sub",
            "Int Subtract",
            BlueprintPinType::Int,
            BlueprintPinType::Int,
            BlueprintPinType::Int,
            &["math", "int", "-", "subtract"],
        ),
        math_value_node(
            "math.int.mul",
            "Int Multiply",
            BlueprintPinType::Int,
            BlueprintPinType::Int,
            BlueprintPinType::Int,
            &["math", "int", "*", "multiply"],
        ),
        math_value_node(
            "math.int.div",
            "Int Divide",
            BlueprintPinType::Int,
            BlueprintPinType::Int,
            BlueprintPinType::Int,
            &["math", "int", "/", "divide"],
        ),
        math_value_node(
            "math.int.mod",
            "Int Modulo",
            BlueprintPinType::Int,
            BlueprintPinType::Int,
            BlueprintPinType::Int,
            &["math", "int", "%", "mod"],
        ),
        math_value_node(
            "math.int.pow",
            "Int Power",
            BlueprintPinType::Int,
            BlueprintPinType::Int,
            BlueprintPinType::Int,
            &["math", "int", "^", "pow", "power"],
        ),
        math_value_node(
            "math.float.add",
            "Float Add",
            BlueprintPinType::Float,
            BlueprintPinType::Float,
            BlueprintPinType::Float,
            &["math", "float", "+", "add"],
        ),
        math_value_node(
            "math.float.sub",
            "Float Subtract",
            BlueprintPinType::Float,
            BlueprintPinType::Float,
            BlueprintPinType::Float,
            &["math", "float", "-", "subtract"],
        ),
        math_value_node(
            "math.float.mul",
            "Float Multiply",
            BlueprintPinType::Float,
            BlueprintPinType::Float,
            BlueprintPinType::Float,
            &["math", "float", "*", "multiply"],
        ),
        math_value_node(
            "math.float.div",
            "Float Divide",
            BlueprintPinType::Float,
            BlueprintPinType::Float,
            BlueprintPinType::Float,
            &["math", "float", "/", "divide"],
        ),
        math_value_node(
            "math.float.mod",
            "Float Modulo",
            BlueprintPinType::Float,
            BlueprintPinType::Float,
            BlueprintPinType::Float,
            &["math", "float", "%", "mod"],
        ),
        math_value_node(
            "math.float.pow",
            "Float Power",
            BlueprintPinType::Float,
            BlueprintPinType::Float,
            BlueprintPinType::Float,
            &["math", "float", "^", "pow", "power"],
        ),
        math_value_node(
            "math.int_float.add",
            "Int + Float",
            BlueprintPinType::Int,
            BlueprintPinType::Float,
            BlueprintPinType::Float,
            &["math", "int", "float", "+", "add"],
        ),
        math_value_node(
            "math.int_float.sub",
            "Int - Float",
            BlueprintPinType::Int,
            BlueprintPinType::Float,
            BlueprintPinType::Float,
            &["math", "int", "float", "-", "subtract"],
        ),
        math_value_node(
            "math.int_float.mul",
            "Int * Float",
            BlueprintPinType::Int,
            BlueprintPinType::Float,
            BlueprintPinType::Float,
            &["math", "int", "float", "*", "multiply"],
        ),
        math_value_node(
            "math.int_float.div",
            "Int / Float",
            BlueprintPinType::Int,
            BlueprintPinType::Float,
            BlueprintPinType::Float,
            &["math", "int", "float", "/", "divide"],
        ),
        math_value_node(
            "math.int_float.mod",
            "Int % Float",
            BlueprintPinType::Int,
            BlueprintPinType::Float,
            BlueprintPinType::Float,
            &["math", "int", "float", "%", "mod"],
        ),
        math_value_node(
            "math.int_float.pow",
            "Int ^ Float",
            BlueprintPinType::Int,
            BlueprintPinType::Float,
            BlueprintPinType::Float,
            &["math", "int", "float", "^", "pow", "power"],
        ),
        math_value_node(
            "string.concat",
            "String Concat",
            BlueprintPinType::String,
            BlueprintPinType::String,
            BlueprintPinType::String,
            &["string", "concat", "add", "+", "text"],
        ),
        compare_value_node(
            "compare.gt",
            "Greater Than",
            BlueprintPinType::Float,
            BlueprintPinType::Float,
            &["compare", "condition", ">", "greater"],
        ),
        compare_value_node(
            "compare.lt",
            "Less Than",
            BlueprintPinType::Float,
            BlueprintPinType::Float,
            &["compare", "condition", "<", "less"],
        ),
        compare_value_node(
            "compare.gte",
            "Greater Or Equal",
            BlueprintPinType::Float,
            BlueprintPinType::Float,
            &["compare", "condition", ">=", "greater", "equal"],
        ),
        compare_value_node(
            "compare.lte",
            "Less Or Equal",
            BlueprintPinType::Float,
            BlueprintPinType::Float,
            &["compare", "condition", "<=", "less", "equal"],
        ),
        compare_value_node(
            "compare.eq",
            "Equal",
            BlueprintPinType::Any,
            BlueprintPinType::Any,
            &["compare", "condition", "==", "equal"],
        ),
        compare_value_node(
            "compare.neq",
            "Not Equal",
            BlueprintPinType::Any,
            BlueprintPinType::Any,
            &["compare", "condition", "!=", "not", "equal"],
        ),
        convert_node(
            "convert.to_bool",
            "To Bool",
            BlueprintPinType::Bool,
            &["convert", "cast", "bool", "value"],
        ),
        convert_node(
            "convert.to_int",
            "To Int",
            BlueprintPinType::Int,
            &["convert", "cast", "int", "value"],
        ),
        convert_node(
            "convert.to_float",
            "To Float",
            BlueprintPinType::Float,
            &["convert", "cast", "float", "value"],
        ),
        convert_node(
            "convert.to_string",
            "To String",
            BlueprintPinType::String,
            &["convert", "cast", "string", "value"],
        ),
        BlueprintNodeDescriptor {
            id: "variables.get_dynamic".to_string(),
            title: "Get Variable (Dynamic)".to_string(),
            category: "Variables".to_string(),
            context: BlueprintNodeContext::Any,
            tags: vec![
                "variable".to_string(),
                "get".to_string(),
                "dynamic".to_string(),
                "l4".to_string(),
            ],
            pins: vec![BlueprintPinDescriptor::data_output(
                "value",
                BlueprintPinType::Any,
            )],
        },
        BlueprintNodeDescriptor {
            id: "variables.set_dynamic".to_string(),
            title: "Set Variable (Dynamic)".to_string(),
            category: "Variables".to_string(),
            context: BlueprintNodeContext::Any,
            tags: vec![
                "variable".to_string(),
                "set".to_string(),
                "dynamic".to_string(),
                "l4".to_string(),
            ],
            pins: vec![
                BlueprintPinDescriptor::exec_input("in"),
                BlueprintPinDescriptor::data_input("value", BlueprintPinType::Any),
                BlueprintPinDescriptor::exec_output("then"),
            ],
        },
        BlueprintNodeDescriptor {
            id: "functions.return".to_string(),
            title: "Function Return".to_string(),
            category: "Functions".to_string(),
            context: BlueprintNodeContext::Any,
            tags: vec![
                "function".to_string(),
                "return".to_string(),
                "exit".to_string(),
                "l4".to_string(),
            ],
            pins: vec![
                BlueprintPinDescriptor::exec_input("in"),
                BlueprintPinDescriptor::data_input("value", BlueprintPinType::Any),
            ],
        },
        BlueprintNodeDescriptor {
            id: "types.struct_make".to_string(),
            title: "Struct Make".to_string(),
            category: "Types".to_string(),
            context: BlueprintNodeContext::Any,
            tags: vec![
                "struct".to_string(),
                "object".to_string(),
                "make".to_string(),
                "l4".to_string(),
            ],
            pins: vec![
                BlueprintPinDescriptor::data_input("fields", BlueprintPinType::Object),
                BlueprintPinDescriptor::data_output("result", BlueprintPinType::Object),
            ],
        },
        BlueprintNodeDescriptor {
            id: "types.enum_value".to_string(),
            title: "Enum Value".to_string(),
            category: "Types".to_string(),
            context: BlueprintNodeContext::Any,
            tags: vec![
                "enum".to_string(),
                "value".to_string(),
                "string".to_string(),
                "l4".to_string(),
            ],
            pins: vec![
                BlueprintPinDescriptor::data_input("value", BlueprintPinType::String),
                BlueprintPinDescriptor::data_output("value", BlueprintPinType::String),
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

fn data_literal_node(
    id: &str,
    title: &str,
    category: &str,
    data_type: BlueprintPinType,
    tags: &[&str],
) -> BlueprintNodeDescriptor {
    BlueprintNodeDescriptor {
        id: id.to_string(),
        title: title.to_string(),
        category: category.to_string(),
        context: BlueprintNodeContext::Any,
        tags: tags.iter().map(|tag| (*tag).to_string()).collect(),
        pins: vec![BlueprintPinDescriptor::data_output("value", data_type)],
    }
}

fn convert_node(
    id: &str,
    title: &str,
    output_type: BlueprintPinType,
    tags: &[&str],
) -> BlueprintNodeDescriptor {
    BlueprintNodeDescriptor {
        id: id.to_string(),
        title: title.to_string(),
        category: "Values".to_string(),
        context: BlueprintNodeContext::Any,
        tags: tags.iter().map(|tag| (*tag).to_string()).collect(),
        pins: vec![
            BlueprintPinDescriptor::data_input("value", BlueprintPinType::Any),
            BlueprintPinDescriptor::data_output("result", output_type),
        ],
    }
}

fn math_value_node(
    id: &str,
    title: &str,
    left_type: BlueprintPinType,
    right_type: BlueprintPinType,
    result_type: BlueprintPinType,
    tags: &[&str],
) -> BlueprintNodeDescriptor {
    BlueprintNodeDescriptor {
        id: id.to_string(),
        title: title.to_string(),
        category: "Values".to_string(),
        context: BlueprintNodeContext::Any,
        tags: tags.iter().map(|tag| (*tag).to_string()).collect(),
        pins: vec![
            BlueprintPinDescriptor::data_input("", left_type),
            BlueprintPinDescriptor::data_input("", right_type),
            BlueprintPinDescriptor::data_output("", result_type),
        ],
    }
}

fn compare_value_node(
    id: &str,
    title: &str,
    left_type: BlueprintPinType,
    right_type: BlueprintPinType,
    tags: &[&str],
) -> BlueprintNodeDescriptor {
    BlueprintNodeDescriptor {
        id: id.to_string(),
        title: title.to_string(),
        category: "Conditions".to_string(),
        context: BlueprintNodeContext::Any,
        tags: tags.iter().map(|tag| (*tag).to_string()).collect(),
        pins: vec![
            BlueprintPinDescriptor::data_input("", left_type),
            BlueprintPinDescriptor::data_input("", right_type),
            BlueprintPinDescriptor::data_output("", BlueprintPinType::Bool),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn math_and_compare_nodes_are_pure_data_nodes() {
        for descriptor_id in [
            "math.int.add",
            "math.float.mul",
            "string.concat",
            "compare.gt",
            "compare.eq",
        ] {
            let descriptor = builtin_node_descriptor(descriptor_id).expect("descriptor");
            assert!(
                descriptor
                    .pins
                    .iter()
                    .all(|pin| pin.kind == BlueprintPinKind::Data),
                "{descriptor_id} should not expose exec pins"
            );
            assert!(
                descriptor
                    .pins
                    .iter()
                    .filter(|pin| pin.direction == BlueprintPinDirection::Input)
                    .all(|pin| pin.name.is_empty()),
                "{descriptor_id} input pins should be visually unnamed"
            );
            assert!(
                descriptor
                    .pins
                    .iter()
                    .filter(|pin| pin.direction == BlueprintPinDirection::Output)
                    .all(|pin| pin.name.is_empty()),
                "{descriptor_id} output pins should be visually unnamed"
            );
        }
    }
}
