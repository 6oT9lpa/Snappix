use crate::layout::LayoutStyles;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ElementKind {
    FlexContainer,
    GridContainer,
    StackContainer,
    Div,
    Text,
    Label,
    Input,
    Textarea,
    Checkbox,
    Image,
    Button,
    Vector,
}

/// UI элемент.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct UiElement {
    pub id: Uuid,
    pub kind: ElementKind,
    pub layout: LayoutStyles,
    pub children: Vec<UiElement>,
    pub properties: std::collections::HashMap<String, serde_json::Value>,
}
