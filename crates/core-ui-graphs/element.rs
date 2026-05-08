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
    #[serde(other)]
    Unknown,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_vector_kind_deserializes_as_unknown() {
        let kind: ElementKind = serde_json::from_str("\"vector\"").expect("deserialize kind");
        assert_eq!(kind, ElementKind::Unknown);
    }

    #[test]
    fn button_kind_round_trips_by_name() {
        let json = serde_json::to_string(&ElementKind::Button).expect("serialize kind");
        assert_eq!(json, "\"button\"");
        let kind: ElementKind = serde_json::from_str(&json).expect("deserialize kind");
        assert_eq!(kind, ElementKind::Button);
    }
}
