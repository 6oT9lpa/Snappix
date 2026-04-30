//! Project management logic.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use image::{ImageFormat, RgbaImage};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use core_blueprint::{
    blueprint_name_for_page, compile_project, BlueprintCompilationResult,
    BlueprintDocument, BlueprintDocumentKind, BlueprintFunctionParameter,
    BlueprintFunctionSignature, BlueprintGraph, BlueprintGraphKind, BlueprintLocalVariable,
    BlueprintNode, BlueprintNodeKind, BlueprintOwner, BlueprintPinType, BlueprintPoint,
    BlueprintProjectApi,
    PageApiDescriptor, ServerApiDescriptor, UiActionDescriptor, UiElementApiDescriptor,
    UiEventDescriptor,
};
use core_ui_graphs::{
    layout::{
        AlignContent, AlignItems, FlexDirection, FlexLayout, FlexWrap, GridLayout, JustifyContent,
        LayoutStyles, SizeValue,
    },
    project::{
        Page as CorePage, PageComment as CorePageComment, PageCommentImage as CorePageCommentImage,
    },
    ElementKind, FormFactor, Os, Platform as CorePlatform, ProjectManifest, ProjectMode, UiElement,
};
use project_manager::{operations, EditorDocumentRef, ProjectFile};

const COMMENT_TITLE_MAX_CHARS: usize = 120;
const COMMENT_BODY_MAX_CHARS: usize = 8_192;

fn truncate_chars(input: &str, max_chars: usize) -> String {
    input.chars().take(max_chars).collect()
}

/// Development mode for the project (UI layer mapping)
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum DevMode {
    #[default]
    Nodes,
    Code,
    Hybrid,
}

impl From<DevMode> for ProjectMode {
    fn from(mode: DevMode) -> Self {
        match mode {
            DevMode::Nodes => ProjectMode::Blueprint,
            DevMode::Code => ProjectMode::Code,
            DevMode::Hybrid => ProjectMode::Hybrid,
        }
    }
}

impl From<ProjectMode> for DevMode {
    fn from(mode: ProjectMode) -> Self {
        match mode {
            ProjectMode::Blueprint => DevMode::Nodes,
            ProjectMode::Code => DevMode::Code,
            ProjectMode::Hybrid => DevMode::Hybrid,
        }
    }
}

/// Target platform (UI layer mapping)
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum Platform {
    #[default]
    Desktop,
    Mobile,
    Web,
}

impl From<Platform> for CorePlatform {
    fn from(platform: Platform) -> Self {
        let os = match platform {
            Platform::Desktop => Os::Windows,
            Platform::Mobile => Os::Android,
            Platform::Web => Os::Windows,
        };
        let form_factor = match platform {
            Platform::Desktop => FormFactor::Desktop,
            Platform::Mobile => FormFactor::Mobile,
            Platform::Web => FormFactor::Web,
        };
        CorePlatform { os, form_factor }
    }
}

/// Page size preset
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum PageSize {
    #[default]
    Desktop,
    Tablet,
    Mobile,
    Custom,
}

impl PageSize {
    pub fn default_size(&self) -> (u32, u32) {
        match self {
            PageSize::Desktop => (1920, 1080),
            PageSize::Tablet => (1024, 768),
            PageSize::Mobile => (375, 812),
            PageSize::Custom => (800, 600),
        }
    }
}

/// Canvas element data for UI layer.
#[derive(Debug, Clone, Default)]
pub struct CanvasElementData {
    pub id: Uuid,
    pub element_type: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub rotation: f32,
    pub properties: serde_json::Value,
}

impl CanvasElementData {
    pub fn from_component_template(element_type: &str, x: f32, y: f32) -> Self {
        let (width, height) = Self::template_size(element_type);
        let mut props = Self::template_properties(element_type);
        props.insert("element_type".to_string(), serde_json::json!(element_type));
        props.insert(
            "display_name".to_string(),
            serde_json::json!(Self::display_name(element_type)),
        );
        props.insert(
            "name".to_string(),
            serde_json::json!(Self::name_seed(element_type)),
        );

        Self {
            id: Uuid::new_v4(),
            element_type: element_type.to_string(),
            x,
            y,
            width,
            height,
            rotation: 0.0,
            properties: serde_json::to_value(props).unwrap_or_else(|_| serde_json::json!({})),
        }
    }

    fn display_name(element_type: &str) -> &'static str {
        match element_type {
            "flex-container" | "flex" => "Flex Container",
            "grid-container" | "grid" => "Grid Container",
            "stack-container" | "stack" => "Stack Container",
            "div" => "Div",
            "text" => "Text",
            "label" => "Label",
            "button" => "Button",
            "input" => "Input",
            "textarea" => "Textarea",
            "checkbox" => "Checkbox",
            "image" => "Image",
            "video" => "Video",
            "audio" => "Audio",
            "select" => "Select",
            "radio" => "Radio",
            "slider" => "Slider",
            "switch" => "Switch",
            _ => "Component",
        }
    }

    fn name_seed(element_type: &str) -> String {
        match element_type {
            "flex-container" => "flex",
            "grid-container" => "grid",
            "stack-container" => "stack",
            other => other,
        }
        .replace('-', "")
        .to_lowercase()
    }

    fn template_size(element_type: &str) -> (f32, f32) {
        match element_type {
            "flex-container" | "stack-container" | "div" => (320.0, 200.0),
            "grid-container" => (360.0, 240.0),
            "text" => (220.0, 40.0),
            "label" => (220.0, 28.0),
            "button" => (140.0, 42.0),
            "input" | "select" => (220.0, 40.0),
            "textarea" => (260.0, 120.0),
            "checkbox" | "radio" | "switch" => (180.0, 32.0),
            "slider" => (220.0, 24.0),
            "image" | "video" | "audio" => (260.0, 160.0),
            _ => (140.0, 100.0),
        }
    }

    fn template_properties(element_type: &str) -> HashMap<String, serde_json::Value> {
        let mut props = HashMap::new();
        props.insert("background".to_string(), serde_json::json!("#ffffff"));
        props.insert("border_color".to_string(), serde_json::json!("#4a4a4a"));
        props.insert("border_width".to_string(), serde_json::json!(0.0));
        props.insert("border_radius".to_string(), serde_json::json!(6.0));
        props.insert(
            "text".to_string(),
            serde_json::json!(Self::display_name(element_type)),
        );
        props.insert("text_color".to_string(), serde_json::json!("#f5f5f5"));
        props.insert("font_size".to_string(), serde_json::json!(14.0));
        props.insert("font_family".to_string(), serde_json::json!("Sans"));
        props.insert("text_wrap".to_string(), serde_json::json!("wrap"));
        props.insert("inherit_text_style".to_string(), serde_json::json!(true));
        props.insert("background_image".to_string(), serde_json::json!(""));
        props.insert("image_src".to_string(), serde_json::json!(""));
        props.insert("locked".to_string(), serde_json::json!(false));
        props.insert("flip_horizontal".to_string(), serde_json::json!(false));
        props.insert("flip_vertical".to_string(), serde_json::json!(false));
        props.insert("positioning".to_string(), serde_json::json!("absolute"));
        props.insert("responsive_mode".to_string(), serde_json::json!("manual"));
        props.insert("container_mode".to_string(), serde_json::json!("absolute"));
        props.insert(
            "allow_absolute_children".to_string(),
            serde_json::json!(false),
        );
        props.insert("layout_padding".to_string(), serde_json::json!(8.0));
        props.insert("layout_padding_left".to_string(), serde_json::json!(8.0));
        props.insert("layout_padding_right".to_string(), serde_json::json!(8.0));
        props.insert("layout_padding_top".to_string(), serde_json::json!(8.0));
        props.insert("layout_padding_bottom".to_string(), serde_json::json!(8.0));
        props.insert("layout_spacing".to_string(), serde_json::json!(8.0));
        props.insert("layout_margin".to_string(), serde_json::json!(0.0));
        props.insert("layout_margin_left".to_string(), serde_json::json!(0.0));
        props.insert("layout_margin_right".to_string(), serde_json::json!(0.0));
        props.insert("layout_margin_top".to_string(), serde_json::json!(0.0));
        props.insert("layout_margin_bottom".to_string(), serde_json::json!(0.0));
        props.insert("layout_order".to_string(), serde_json::json!(0.0));
        props.insert("stack_alignment".to_string(), serde_json::json!("stretch"));
        props.insert("justify_items".to_string(), serde_json::json!("stretch"));
        props.insert(
            "justify_content".to_string(),
            serde_json::json!("flex-start"),
        );
        props.insert("align_items".to_string(), serde_json::json!("stretch"));
        props.insert("align_content".to_string(), serde_json::json!("stretch"));
        props.insert(
            "place_items".to_string(),
            serde_json::json!("stretch stretch"),
        );
        props.insert("flex_flow".to_string(), serde_json::json!("column nowrap"));
        props.insert(
            "grid_template_columns".to_string(),
            serde_json::json!("1fr 1fr"),
        );
        props.insert(
            "grid_template_rows".to_string(),
            serde_json::json!("auto auto"),
        );
        props.insert("grid_template_areas".to_string(), serde_json::json!(""));

        match element_type {
            "stack-container" | "stack" => {
                props.insert("background".to_string(), serde_json::json!("#ffffff"));
                props.insert("container_mode".to_string(), serde_json::json!("stack"));
                props.insert("display".to_string(), serde_json::json!("block"));
                props.insert(
                    "positioning_mode".to_string(),
                    serde_json::json!("absolute-children"),
                );
            }
            "grid-container" | "grid" => {
                props.insert("background".to_string(), serde_json::json!("#ffffff"));
                props.insert("container_mode".to_string(), serde_json::json!("grid"));
                props.insert("display".to_string(), serde_json::json!("grid"));
                props.insert(
                    "responsive_mode".to_string(),
                    serde_json::json!("layout-managed"),
                );
                props.insert("grid_columns".to_string(), serde_json::json!("1fr 1fr"));
                props.insert("grid_rows".to_string(), serde_json::json!("auto auto"));
                props.insert("grid_auto_flow".to_string(), serde_json::json!("row"));
                props.insert(
                    "positioning_mode".to_string(),
                    serde_json::json!("grid-only"),
                );
            }
            "flex-container" | "flex" => {
                props.insert("background".to_string(), serde_json::json!("#ffffff"));
                props.insert("container_mode".to_string(), serde_json::json!("flex"));
                props.insert("display".to_string(), serde_json::json!("flex"));
                props.insert(
                    "responsive_mode".to_string(),
                    serde_json::json!("layout-managed"),
                );
                props.insert("flex_direction".to_string(), serde_json::json!("column"));
                props.insert("flex_wrap".to_string(), serde_json::json!("nowrap"));
                props.insert(
                    "justify_content".to_string(),
                    serde_json::json!("flex-start"),
                );
                props.insert("align_items".to_string(), serde_json::json!("stretch"));
                props.insert(
                    "positioning_mode".to_string(),
                    serde_json::json!("flex-only"),
                );
            }
            "div" => {
                props.insert("background".to_string(), serde_json::json!("#ffffff"));
                props.remove("display");
                props.insert("text".to_string(), serde_json::json!(""));
                props.insert("container_mode".to_string(), serde_json::json!("absolute"));
                props.insert(
                    "positioning_mode".to_string(),
                    serde_json::json!("absolute-children"),
                );
            }
            "text" => {
                props.insert("background".to_string(), serde_json::json!("#0000"));
                props.insert("text".to_string(), serde_json::json!("Text"));
                props.insert("font_size".to_string(), serde_json::json!(20.0));
                props.insert("border_width".to_string(), serde_json::json!(0.0));
                props.insert("wrap_text".to_string(), serde_json::json!(true));
            }
            "label" => {
                props.insert("background".to_string(), serde_json::json!("#0000"));
                props.insert("text".to_string(), serde_json::json!("Label"));
                props.insert("font_size".to_string(), serde_json::json!(18.0));
                props.insert("border_width".to_string(), serde_json::json!(0.0));
                props.insert("single_line".to_string(), serde_json::json!(true));
                props.insert("wrap_text".to_string(), serde_json::json!(false));
                props.insert("text_wrap".to_string(), serde_json::json!("nowrap"));
            }
            "button" => {
                props.insert("background".to_string(), serde_json::json!("#2f7bff"));
                props.insert("text".to_string(), serde_json::json!("Button"));
                props.insert("border_radius".to_string(), serde_json::json!(8.0));
                props.insert("text_wrap".to_string(), serde_json::json!("nowrap"));
            }
            "input" => {
                props.insert("background".to_string(), serde_json::json!("#151515"));
                props.insert("text".to_string(), serde_json::json!(""));
                props.insert("placeholder".to_string(), serde_json::json!("Type here"));
                props.insert("focus_on_click".to_string(), serde_json::json!(true));
                props.insert("single_line".to_string(), serde_json::json!(true));
                props.insert("text_wrap".to_string(), serde_json::json!("nowrap"));
            }
            "textarea" => {
                props.insert("background".to_string(), serde_json::json!("#151515"));
                props.insert("text".to_string(), serde_json::json!(""));
                props.insert(
                    "placeholder".to_string(),
                    serde_json::json!("Type multiple lines"),
                );
                props.insert("focus_on_click".to_string(), serde_json::json!(true));
                props.insert("single_line".to_string(), serde_json::json!(false));
                props.insert("wrap_text".to_string(), serde_json::json!(true));
            }
            "checkbox" => {
                props.insert("text".to_string(), serde_json::json!("Checkbox"));
                props.insert("checked".to_string(), serde_json::json!(false));
                props.insert("checkbox_box_side".to_string(), serde_json::json!("left"));
                props.insert(
                    "checkbox_check_color".to_string(),
                    serde_json::json!("#f5f5f5"),
                );
                props.insert(
                    "checkbox_box_color".to_string(),
                    serde_json::json!("#151515"),
                );
                props.insert(
                    "checkbox_box_border_color".to_string(),
                    serde_json::json!("#4a4a4a"),
                );
                props.insert(
                    "checkbox_box_border_width".to_string(),
                    serde_json::json!(1.0),
                );
                props.insert(
                    "checkbox_space_between".to_string(),
                    serde_json::json!(false),
                );
            }
            "select" => {
                props.insert("text".to_string(), serde_json::json!("Select..."));
            }
            "radio" => {
                props.insert("text".to_string(), serde_json::json!("Radio"));
            }
            "slider" => {
                props.insert("value".to_string(), serde_json::json!(50));
            }
            "switch" => {
                props.insert("value".to_string(), serde_json::json!(false));
            }
            "image" | "video" | "audio" => {
                props.insert("background".to_string(), serde_json::json!("#141820"));
                props.insert(
                    "text".to_string(),
                    serde_json::json!(if element_type == "image" {
                        "No image"
                    } else {
                        Self::display_name(element_type)
                    }),
                );
                if element_type == "image" {
                    props.insert("src".to_string(), serde_json::json!(""));
                    props.insert("alt".to_string(), serde_json::json!(""));
                    props.insert("image_src".to_string(), serde_json::json!(""));
                }
            }
            _ => {}
        }

        props
    }

    fn kind_for_type(element_type: &str) -> ElementKind {
        match element_type {
            "flex-container" | "flex" => ElementKind::FlexContainer,
            "grid-container" | "grid" => ElementKind::GridContainer,
            "stack-container" | "stack" => ElementKind::StackContainer,
            "div" => ElementKind::Div,
            "text" => ElementKind::Text,
            "label" => ElementKind::Label,
            "input" | "select" | "radio" | "slider" | "switch" => ElementKind::Input,
            "textarea" => ElementKind::Textarea,
            "checkbox" => ElementKind::Checkbox,
            "image" | "video" | "audio" => ElementKind::Image,
            "button" => ElementKind::Button,
            _ => ElementKind::FlexContainer,
        }
    }

    fn layout_for_type(element_type: &str) -> LayoutStyles {
        match element_type {
            "flex-container" | "flex" => LayoutStyles::Flex(FlexLayout {
                direction: FlexDirection::Column,
                wrap: FlexWrap::NoWrap,
                justify_content: JustifyContent::FlexStart,
                align_items: AlignItems::Stretch,
                align_content: AlignContent::Stretch,
                gap: Some(SizeValue::Pixels(8.0)),
            }),
            "grid-container" | "grid" => LayoutStyles::Grid(GridLayout {
                columns: vec![SizeValue::Fraction(1.0), SizeValue::Fraction(1.0)],
                rows: vec![SizeValue::Auto, SizeValue::Auto],
                gap: Some(SizeValue::Pixels(8.0)),
            }),
            "stack-container" | "stack" | "div" => LayoutStyles::Block { overflow: None },
            _ => LayoutStyles::Block { overflow: None },
        }
    }

    /// Convert to core UiElement.
    pub fn to_ui_element(&self) -> UiElement {
        let mut props = Self::template_properties(self.element_type.as_str());
        props.insert(
            "element_type".to_string(),
            serde_json::json!(self.element_type.clone()),
        );
        props.insert("x".to_string(), serde_json::json!(self.x));
        props.insert("y".to_string(), serde_json::json!(self.y));
        props.insert("width".to_string(), serde_json::json!(self.width));
        props.insert("height".to_string(), serde_json::json!(self.height));
        props.insert("rotation".to_string(), serde_json::json!(self.rotation));

        if let Some(obj) = self.properties.as_object() {
            for (k, v) in obj {
                props.insert(k.clone(), v.clone());
            }
        }

        UiElement {
            id: self.id,
            kind: Self::kind_for_type(self.element_type.as_str()),
            layout: Self::layout_for_type(self.element_type.as_str()),
            children: Vec::new(),
            properties: props,
        }
    }

    /// Create from core UiElement.
    pub fn from_ui_element(element: &UiElement) -> Self {
        let fallback_type = match element.kind {
            ElementKind::FlexContainer => "flex-container",
            ElementKind::GridContainer => "grid-container",
            ElementKind::StackContainer => "stack-container",
            ElementKind::Div => "div",
            ElementKind::Text => "text",
            ElementKind::Label => "label",
            ElementKind::Input => "input",
            ElementKind::Textarea => "textarea",
            ElementKind::Checkbox => "checkbox",
            ElementKind::Image => "image",
            ElementKind::Button => "button",
            _ => "div",
        };

        let raw_element_type = element
            .properties
            .get("element_type")
            .and_then(|v| v.as_str())
            .unwrap_or(fallback_type);
        let element_type = match raw_element_type {
            "vector" | "vector-line" | "vector-rect" | "vector-ellipse" | "vector-pen" => "div",
            other => other,
        }
        .to_string();

        let x = element
            .properties
            .get("x")
            .and_then(|v| v.as_f64())
            .map(|v| v as f32)
            .unwrap_or(0.0);
        let y = element
            .properties
            .get("y")
            .and_then(|v| v.as_f64())
            .map(|v| v as f32)
            .unwrap_or(0.0);
        let width = element
            .properties
            .get("width")
            .and_then(|v| v.as_f64())
            .map(|v| v as f32)
            .unwrap_or(100.0);
        let height = element
            .properties
            .get("height")
            .and_then(|v| v.as_f64())
            .map(|v| v as f32)
            .unwrap_or(100.0);
        let rotation = element
            .properties
            .get("rotation")
            .and_then(|v| v.as_f64())
            .map(|v| v as f32)
            .unwrap_or(0.0);

        Self {
            id: element.id,
            element_type,
            x,
            y,
            width,
            height,
            rotation,
            properties: serde_json::to_value(&element.properties)
                .unwrap_or_else(|_| serde_json::json!({})),
        }
    }
}

fn build_page_root(width: u32, height: u32) -> UiElement {
    let mut root_props = HashMap::new();
    root_props.insert("width".to_string(), serde_json::json!(width));
    root_props.insert("height".to_string(), serde_json::json!(height));
    root_props.insert("element_type".to_string(), serde_json::json!("root-canvas"));
    root_props.insert("display_name".to_string(), serde_json::json!("Canvas Root"));

    UiElement {
        id: Uuid::new_v4(),
        kind: ElementKind::FlexContainer,
        layout: LayoutStyles::Flex(FlexLayout {
            direction: FlexDirection::Column,
            wrap: FlexWrap::NoWrap,
            justify_content: JustifyContent::FlexStart,
            align_items: AlignItems::Stretch,
            align_content: AlignContent::Stretch,
            gap: None,
        }),
        children: Vec::new(),
        properties: root_props,
    }
}

fn page_size(page: &CorePage) -> (u32, u32) {
    let Some(root) = page.children.first() else {
        return (1920, 1080);
    };

    let width = root
        .properties
        .get("width")
        .and_then(|v| v.as_u64().or_else(|| v.as_f64().map(|n| n as u64)))
        .map(|v| v as u32)
        .unwrap_or(1920);
    let height = root
        .properties
        .get("height")
        .and_then(|v| v.as_u64().or_else(|| v.as_f64().map(|n| n as u64)))
        .map(|v| v as u32)
        .unwrap_or(1080);

    (width, height)
}

fn collect_elements_recursive<'a>(elements: &'a [UiElement], out: &mut Vec<&'a UiElement>) {
    for element in elements {
        out.push(element);
        collect_elements_recursive(&element.children, out);
    }
}

fn find_element_recursive(elements: &[UiElement], id: Uuid) -> Option<&UiElement> {
    for element in elements {
        if element.id == id {
            return Some(element);
        }
        if let Some(found) = find_element_recursive(&element.children, id) {
            return Some(found);
        }
    }
    None
}

fn find_element_recursive_mut(elements: &mut [UiElement], id: Uuid) -> Option<&mut UiElement> {
    for element in elements {
        if element.id == id {
            return Some(element);
        }
        if let Some(found) = find_element_recursive_mut(&mut element.children, id) {
            return Some(found);
        }
    }
    None
}

fn retain_elements_recursive(elements: &mut Vec<UiElement>, remove_set: &HashSet<Uuid>) -> bool {
    let before = elements.len();
    elements.retain(|element| !remove_set.contains(&element.id));
    let mut removed = before != elements.len();
    for element in elements {
        removed |= retain_elements_recursive(&mut element.children, remove_set);
    }
    removed
}

fn get_string_property(element: &UiElement, key: &str) -> Option<String> {
    element
        .properties
        .get(key)
        .and_then(|value| value.as_str())
        .map(|value| value.to_string())
}

fn parse_parent_property(element: &UiElement) -> Option<Uuid> {
    get_string_property(element, "parent_id").and_then(|value| Uuid::parse_str(&value).ok())
}

fn parse_parent_property_from_canvas(element: &CanvasElementData) -> Option<Uuid> {
    element
        .properties
        .as_object()
        .and_then(|props| props.get("parent_id"))
        .and_then(|value| value.as_str())
        .and_then(|value| Uuid::parse_str(value).ok())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ContainerMode {
    Absolute,
    Stack,
    Flex,
    Grid,
}

impl ContainerMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Absolute => "absolute",
            Self::Stack => "stack",
            Self::Flex => "flex",
            Self::Grid => "grid",
        }
    }

    fn is_managed(self) -> bool {
        !matches!(self, Self::Absolute)
    }
}

#[derive(Debug, Clone, Copy)]
enum GridTrack {
    Fraction(f32),
    Pixels(f32),
    Percent(f32),
    Auto,
}

fn canvas_property_map(
    element: &CanvasElementData,
) -> Option<&serde_json::Map<String, serde_json::Value>> {
    element.properties.as_object()
}

fn canvas_prop_str(element: &CanvasElementData, key: &str, default: &str) -> String {
    canvas_property_map(element)
        .and_then(|props| props.get(key))
        .and_then(|value| value.as_str())
        .unwrap_or(default)
        .to_string()
}

fn canvas_prop_f32(element: &CanvasElementData, key: &str, default: f32) -> f32 {
    canvas_property_map(element)
        .and_then(|props| props.get(key))
        .and_then(|value| value.as_f64())
        .map(|value| value as f32)
        .unwrap_or(default)
}

#[derive(Debug, Clone, Copy)]
struct LayoutEdges {
    left: f32,
    right: f32,
    top: f32,
    bottom: f32,
}

impl LayoutEdges {
    fn from_canvas(element: &CanvasElementData, prefix: &str, default: f32) -> Self {
        let fallback = canvas_prop_f32(element, prefix, default).max(0.0);
        Self {
            left: canvas_prop_f32(element, &format!("{prefix}_left"), fallback).max(0.0),
            right: canvas_prop_f32(element, &format!("{prefix}_right"), fallback).max(0.0),
            top: canvas_prop_f32(element, &format!("{prefix}_top"), fallback).max(0.0),
            bottom: canvas_prop_f32(element, &format!("{prefix}_bottom"), fallback).max(0.0),
        }
    }

    fn horizontal(self) -> f32 {
        self.left + self.right
    }

    fn vertical(self) -> f32 {
        self.top + self.bottom
    }
}

fn canvas_prop_bool(element: &CanvasElementData, key: &str, default: bool) -> bool {
    canvas_property_map(element)
        .and_then(|props| props.get(key))
        .and_then(|value| value.as_bool())
        .unwrap_or(default)
}

fn normalize_container_mode(value: &str) -> ContainerMode {
    match value.trim().to_ascii_lowercase().as_str() {
        "stack" => ContainerMode::Stack,
        "flex" => ContainerMode::Flex,
        "grid" => ContainerMode::Grid,
        _ => ContainerMode::Absolute,
    }
}

fn normalize_stack_alignment(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "start" | "flex-start" => "start",
        "center" => "center",
        "end" | "flex-end" => "end",
        _ => "stretch",
    }
    .to_string()
}

fn normalize_flex_direction(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "row" => "row",
        _ => "column",
    }
    .to_string()
}

fn normalize_flex_wrap(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "wrap" => "wrap",
        "wrap-reverse" => "wrap-reverse",
        _ => "nowrap",
    }
    .to_string()
}

fn normalize_item_alignment(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "start" | "flex-start" => "flex-start",
        "center" => "center",
        "end" | "flex-end" => "flex-end",
        "baseline" => "baseline",
        _ => "stretch",
    }
    .to_string()
}

fn normalize_justify_content(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "center" => "center",
        "end" | "flex-end" => "flex-end",
        "space-between" => "space-between",
        "space-around" => "space-around",
        "space-evenly" => "space-evenly",
        _ => "flex-start",
    }
    .to_string()
}

fn normalize_align_content(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "center" => "center",
        "end" | "flex-end" => "flex-end",
        "space-between" => "space-between",
        "space-around" => "space-around",
        "stretch" => "stretch",
        _ => "flex-start",
    }
    .to_string()
}

fn parse_place_items(
    value: &str,
    fallback_justify: &str,
    fallback_align: &str,
) -> (String, String) {
    let mut tokens = value.split_whitespace();
    let justify = tokens
        .next()
        .map(normalize_item_alignment)
        .unwrap_or_else(|| fallback_justify.to_string());
    let align = tokens
        .next()
        .map(normalize_item_alignment)
        .unwrap_or_else(|| fallback_align.to_string());
    (justify, align)
}

fn parse_flex_flow(value: &str, fallback_direction: &str, fallback_wrap: &str) -> (String, String) {
    let mut direction = fallback_direction.to_string();
    let mut wrap = fallback_wrap.to_string();
    for token in value.split_whitespace() {
        let lower = token.trim().to_ascii_lowercase();
        if lower == "row" || lower == "column" {
            direction = normalize_flex_direction(&lower);
        } else if matches!(lower.as_str(), "nowrap" | "wrap" | "wrap-reverse") {
            wrap = normalize_flex_wrap(&lower);
        }
    }
    (direction, wrap)
}

fn effective_container_mode_for_type_and_props(
    element_type: &str,
    props: Option<&serde_json::Map<String, serde_json::Value>>,
) -> ContainerMode {
    match element_type {
        "stack-container" | "stack" => ContainerMode::Stack,
        "flex-container" | "flex" => ContainerMode::Flex,
        "grid-container" | "grid" => ContainerMode::Grid,
        "div" => props
            .and_then(|map| map.get("container_mode"))
            .and_then(|value| value.as_str())
            .map(normalize_container_mode)
            .unwrap_or(ContainerMode::Absolute),
        _ => ContainerMode::Absolute,
    }
}

fn effective_container_mode_for_canvas(element: &CanvasElementData) -> ContainerMode {
    effective_container_mode_for_type_and_props(
        element.element_type.as_str(),
        canvas_property_map(element),
    )
}

fn container_allows_absolute_children(element: &CanvasElementData) -> bool {
    let mode = effective_container_mode_for_canvas(element);
    mode == ContainerMode::Absolute
        || (mode == ContainerMode::Stack
            && canvas_prop_bool(element, "allow_absolute_children", false))
}

fn child_uses_absolute_positioning(element: &CanvasElementData) -> bool {
    canvas_prop_str(element, "positioning", "absolute")
        .trim()
        .eq_ignore_ascii_case("absolute")
}

fn parse_grid_track(token: &str) -> Option<GridTrack> {
    let trimmed = token.trim();
    if trimmed.is_empty() {
        return None;
    }

    let lower = trimmed.to_ascii_lowercase();
    if lower == "auto" {
        return Some(GridTrack::Auto);
    }
    if let Some(value) = lower.strip_suffix("fr") {
        let parsed = value.parse::<f32>().ok().filter(|value| *value > 0.0)?;
        return Some(GridTrack::Fraction(parsed));
    }
    if let Some(value) = lower.strip_suffix("px") {
        let parsed = value.parse::<f32>().ok().filter(|value| *value >= 0.0)?;
        return Some(GridTrack::Pixels(parsed));
    }
    if let Some(value) = lower.strip_suffix('%') {
        let parsed = value.parse::<f32>().ok().filter(|value| *value >= 0.0)?;
        return Some(GridTrack::Percent(parsed / 100.0));
    }
    lower
        .parse::<f32>()
        .ok()
        .filter(|value| *value >= 0.0)
        .map(GridTrack::Pixels)
}

fn parse_grid_tracks(value: &str, fallback: &str) -> Vec<GridTrack> {
    let parsed: Vec<GridTrack> = value
        .split_whitespace()
        .filter_map(parse_grid_track)
        .collect();
    if parsed.is_empty() {
        fallback
            .split_whitespace()
            .filter_map(parse_grid_track)
            .collect()
    } else {
        parsed
    }
}

fn resolve_grid_tracks(
    tracks: &[GridTrack],
    available: f32,
    gap: f32,
    auto_mins: &[f32],
) -> Vec<f32> {
    if tracks.is_empty() {
        return Vec::new();
    }

    let total_gap = gap.max(0.0) * (tracks.len().saturating_sub(1) as f32);
    let available_without_gap = (available - total_gap).max(0.0);

    let mut sizes = vec![0.0; tracks.len()];
    let mut fixed_total = 0.0_f32;
    let mut auto_total = 0.0_f32;
    let mut auto_indexes = Vec::new();
    let mut fraction_total = 0.0_f32;

    for (idx, track) in tracks.iter().enumerate() {
        match track {
            GridTrack::Pixels(value) => {
                sizes[idx] = (*value).max(0.0);
                fixed_total += sizes[idx];
            }
            GridTrack::Percent(value) => {
                sizes[idx] = (available_without_gap * value.max(0.0)).max(0.0);
                fixed_total += sizes[idx];
            }
            GridTrack::Auto => {
                sizes[idx] = auto_mins.get(idx).copied().unwrap_or(0.0).max(0.0);
                auto_total += sizes[idx];
                auto_indexes.push(idx);
            }
            GridTrack::Fraction(value) => {
                fraction_total += (*value).max(0.0);
            }
        }
    }

    let mut remaining = (available_without_gap - fixed_total - auto_total).max(0.0);
    if fraction_total > 0.0 {
        for (idx, track) in tracks.iter().enumerate() {
            if let GridTrack::Fraction(weight) = track {
                sizes[idx] = remaining * (*weight).max(0.0) / fraction_total;
            }
        }
    } else if !auto_indexes.is_empty() && remaining > 0.0 {
        let extra = remaining / auto_indexes.len() as f32;
        for idx in auto_indexes {
            sizes[idx] += extra;
        }
        remaining = 0.0;
    }

    if remaining > 0.0 && fraction_total == 0.0 && auto_total == 0.0 {
        let extra = remaining / tracks.len() as f32;
        for size in &mut sizes {
            *size += extra;
        }
    }

    sizes.into_iter().map(|value| value.max(1.0)).collect()
}

fn layout_sequence_offsets(
    alignment: &str,
    free_space: f32,
    item_count: usize,
    base_gap: f32,
) -> (f32, f32) {
    if item_count == 0 {
        return (0.0, base_gap.max(0.0));
    }

    let free_space = free_space.max(0.0);
    match alignment {
        "center" => (free_space / 2.0, base_gap.max(0.0)),
        "flex-end" => (free_space, base_gap.max(0.0)),
        "space-between" if item_count > 1 => (
            0.0,
            base_gap.max(0.0) + free_space / (item_count - 1) as f32,
        ),
        "space-around" => {
            let extra = free_space / item_count as f32;
            (extra / 2.0, base_gap.max(0.0) + extra)
        }
        "space-evenly" => {
            let extra = free_space / (item_count + 1) as f32;
            (extra, base_gap.max(0.0) + extra)
        }
        _ => (0.0, base_gap.max(0.0)),
    }
}

fn element_type_name(element: &UiElement) -> String {
    get_string_property(element, "element_type").unwrap_or_else(|| {
        match element.kind {
            ElementKind::FlexContainer => "flex-container",
            ElementKind::GridContainer => "grid-container",
            ElementKind::StackContainer => "stack-container",
            ElementKind::Div => "div",
            ElementKind::Text => "text",
            ElementKind::Label => "label",
            ElementKind::Input => "input",
            ElementKind::Textarea => "textarea",
            ElementKind::Checkbox => "checkbox",
            ElementKind::Image => "image",
            ElementKind::Button => "button",
            _ => "component",
        }
        .to_string()
    })
}

fn element_display_name(element: &UiElement) -> String {
    get_string_property(element, "name")
        .or_else(|| get_string_property(element, "display_name"))
        .unwrap_or_else(|| element_type_name(element))
}

fn blueprint_events_for_element_type(element_type: &str) -> Vec<UiEventDescriptor> {
    match element_type {
        "button" => vec![
            ui_event_descriptor("clicked", "Clicked"),
            ui_event_descriptor("hovered", "Hovered"),
            ui_event_descriptor("pressed", "Pressed"),
            ui_event_descriptor("released", "Released"),
            ui_event_descriptor("focused", "Focused"),
            ui_event_descriptor("blurred", "Blurred"),
        ],
        "checkbox" | "radio" | "switch" => vec![
            ui_event_descriptor("clicked", "Clicked"),
            ui_event_descriptor("changed", "Changed"),
        ],
        "image" | "video" | "audio" => vec![
            ui_event_descriptor("clicked", "Clicked"),
            ui_event_descriptor("hovered", "Hovered"),
        ],
        "input" | "textarea" | "select" | "slider" => vec![
            ui_event_descriptor("changed", "Changed"),
            ui_event_descriptor("focused", "Focused"),
            ui_event_descriptor("blurred", "Blurred"),
        ],
        _ => Vec::new(),
    }
}

fn ui_event_descriptor(name: &str, display_name: &str) -> UiEventDescriptor {
    UiEventDescriptor {
        name: name.to_string(),
        display_name: display_name.to_string(),
    }
}

fn blueprint_actions_for_element_type(element_type: &str) -> Vec<UiActionDescriptor> {
    let text_param = BlueprintFunctionParameter {
        name: "text".to_string(),
        data_type: BlueprintPinType::String,
    };
    let opacity_param = BlueprintFunctionParameter {
        name: "opacity".to_string(),
        data_type: BlueprintPinType::Float,
    };
    let color_param = BlueprintFunctionParameter {
        name: "color".to_string(),
        data_type: BlueprintPinType::Color,
    };

    match element_type {
        "text" | "label" | "button" | "input" | "textarea" | "checkbox" | "radio" | "select" => {
            vec![
                UiActionDescriptor {
                    name: "set_text".to_string(),
                    display_name: "Set Text".to_string(),
                    parameters: vec![text_param],
                    return_type: BlueprintPinType::Void,
                },
                UiActionDescriptor {
                    name: "set_opacity".to_string(),
                    display_name: "Set Opacity".to_string(),
                    parameters: vec![opacity_param.clone()],
                    return_type: BlueprintPinType::Void,
                },
                UiActionDescriptor {
                    name: "set_background_color".to_string(),
                    display_name: "Set Background Color".to_string(),
                    parameters: vec![color_param.clone()],
                    return_type: BlueprintPinType::Void,
                },
            ]
        }
        "div" | "flex-container" | "grid-container" | "stack-container" | "image" => vec![
            UiActionDescriptor {
                name: "set_opacity".to_string(),
                display_name: "Set Opacity".to_string(),
                parameters: vec![opacity_param],
                return_type: BlueprintPinType::Void,
            },
            UiActionDescriptor {
                name: "set_background_color".to_string(),
                display_name: "Set Background Color".to_string(),
                parameters: vec![color_param],
                return_type: BlueprintPinType::Void,
            },
        ],
        _ => vec![UiActionDescriptor {
            name: "set_opacity".to_string(),
            display_name: "Set Opacity".to_string(),
            parameters: vec![opacity_param],
            return_type: BlueprintPinType::Void,
        }],
    }
}

fn normalized_image_asset_path(path: &str, allow_ico: bool) -> String {
    let trimmed = normalize_asset_path(path);
    if trimmed.is_empty() {
        return String::new();
    }

    if !is_project_image_asset(&trimmed) {
        return String::new();
    }

    let Some(extension) = Path::new(&trimmed)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
    else {
        return String::new();
    };

    let allowed = matches!(extension.as_str(), "png" | "jpg" | "jpeg" | "svg" | "webp")
        || (allow_ico && extension == "ico");
    if allowed {
        trimmed
    } else {
        String::new()
    }
}

fn normalize_asset_path(path: &str) -> String {
    path.trim().replace('\\', "/")
}

fn is_project_image_asset(path: &str) -> bool {
    let normalized = normalize_asset_path(path);
    normalized.starts_with("assets/images/") && !normalized.contains("..")
}

fn project_images_dir_from_assets_root(root: &Path) -> PathBuf {
    root.join("assets").join("images")
}

fn project_image_asset_path(file_name: &str) -> String {
    Path::new("assets")
        .join("images")
        .join(file_name)
        .to_string_lossy()
        .replace('\\', "/")
}

fn hash_bytes_fnv1a(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn remove_parent_property(element: &mut UiElement) {
    element.properties.remove("parent_id");
}

fn collect_image_paths_from_elements(elements: &[UiElement], out: &mut HashSet<String>) {
    for element in elements {
        if let Some(value) = get_string_property(element, "image_src") {
            let normalized = normalize_asset_path(&value);
            if is_project_image_asset(&normalized) {
                out.insert(normalized);
            }
        }
        if let Some(value) = get_string_property(element, "background_image") {
            let normalized = normalize_asset_path(&value);
            if is_project_image_asset(&normalized) {
                out.insert(normalized);
            }
        }
        if !element.children.is_empty() {
            collect_image_paths_from_elements(&element.children, out);
        }
    }
}

fn collect_descendant_ids(
    parent_id: Uuid,
    children_map: &HashMap<Uuid, Vec<Uuid>>,
    out: &mut Vec<Uuid>,
) {
    if let Some(children) = children_map.get(&parent_id) {
        for child_id in children {
            out.push(*child_id);
            collect_descendant_ids(*child_id, children_map, out);
        }
    }
}

fn rotate_vector(x: f32, y: f32, rotation_deg: f32) -> (f32, f32) {
    let radians = rotation_deg.to_radians();
    let cos_v = radians.cos();
    let sin_v = radians.sin();
    (x * cos_v - y * sin_v, x * sin_v + y * cos_v)
}

fn rotated_bounding_box(width: f32, height: f32, rotation_deg: f32) -> (f32, f32) {
    let radians = rotation_deg.to_radians();
    let abs_cos = radians.cos().abs();
    let abs_sin = radians.sin().abs();
    (
        width * abs_cos + height * abs_sin,
        width * abs_sin + height * abs_cos,
    )
}

fn clamp_to_parent_bounds(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    child_rotation: f32,
    parent_x: f32,
    parent_y: f32,
    parent_width: f32,
    parent_height: f32,
    parent_rotation: f32,
) -> (f32, f32, f32, f32) {
    let safe_parent_width = parent_width.max(1.0);
    let safe_parent_height = parent_height.max(1.0);
    let mut width = width.max(1.0);
    let mut height = height.max(1.0);

    let relative_rotation = normalize_rotation(child_rotation - parent_rotation);
    let (bbox_w, bbox_h) = rotated_bounding_box(width, height, relative_rotation);
    let scale = (safe_parent_width / bbox_w.max(1.0))
        .min(safe_parent_height / bbox_h.max(1.0))
        .min(1.0);
    width = (width * scale).max(1.0);
    height = (height * scale).max(1.0);

    let (bbox_w, bbox_h) = rotated_bounding_box(width, height, relative_rotation);
    let parent_center_x = parent_x + safe_parent_width / 2.0;
    let parent_center_y = parent_y + safe_parent_height / 2.0;
    let child_center_x = x + width / 2.0;
    let child_center_y = y + height / 2.0;
    let (local_center_x, local_center_y) = rotate_vector(
        child_center_x - parent_center_x,
        child_center_y - parent_center_y,
        -parent_rotation,
    );

    let min_local_x = -safe_parent_width / 2.0 + bbox_w / 2.0;
    let max_local_x = (safe_parent_width / 2.0 - bbox_w / 2.0).max(min_local_x);
    let min_local_y = -safe_parent_height / 2.0 + bbox_h / 2.0;
    let max_local_y = (safe_parent_height / 2.0 - bbox_h / 2.0).max(min_local_y);

    let clamped_local_x = local_center_x.clamp(min_local_x, max_local_x);
    let clamped_local_y = local_center_y.clamp(min_local_y, max_local_y);
    let (world_offset_x, world_offset_y) =
        rotate_vector(clamped_local_x, clamped_local_y, parent_rotation);
    let clamped_center_x = parent_center_x + world_offset_x;
    let clamped_center_y = parent_center_y + world_offset_y;

    (
        clamped_center_x - width / 2.0,
        clamped_center_y - height / 2.0,
        width,
        height,
    )
}

fn normalize_rotation(rotation: f32) -> f32 {
    let mut normalized = rotation;
    while normalized > 180.0 {
        normalized -= 360.0;
    }
    while normalized < -180.0 {
        normalized += 360.0;
    }
    normalized
}

fn transform_descendant_geometry_with_parent(
    child_x: f32,
    child_y: f32,
    child_width: f32,
    child_height: f32,
    child_rotation: f32,
    old_parent_x: f32,
    old_parent_y: f32,
    old_parent_width: f32,
    old_parent_height: f32,
    old_parent_rotation: f32,
    new_parent_x: f32,
    new_parent_y: f32,
    new_parent_width: f32,
    new_parent_height: f32,
    new_parent_rotation: f32,
) -> (f32, f32, f32, f32, f32) {
    let safe_old_width = old_parent_width.max(1.0);
    let safe_old_height = old_parent_height.max(1.0);
    let safe_new_width = new_parent_width.max(1.0);
    let safe_new_height = new_parent_height.max(1.0);

    let scale_x = safe_new_width / safe_old_width;
    let scale_y = safe_new_height / safe_old_height;

    let old_parent_center_x = old_parent_x + safe_old_width / 2.0;
    let old_parent_center_y = old_parent_y + safe_old_height / 2.0;
    let new_parent_center_x = new_parent_x + safe_new_width / 2.0;
    let new_parent_center_y = new_parent_y + safe_new_height / 2.0;
    let child_center_x = child_x + child_width / 2.0;
    let child_center_y = child_y + child_height / 2.0;
    let (local_center_x, local_center_y) = rotate_vector(
        child_center_x - old_parent_center_x,
        child_center_y - old_parent_center_y,
        -old_parent_rotation,
    );
    let scaled_local_x = local_center_x * scale_x;
    let scaled_local_y = local_center_y * scale_y;
    let (world_offset_x, world_offset_y) =
        rotate_vector(scaled_local_x, scaled_local_y, new_parent_rotation);
    let new_center_x = new_parent_center_x + world_offset_x;
    let new_center_y = new_parent_center_y + world_offset_y;

    let delta_rotation = normalize_rotation(new_parent_rotation - old_parent_rotation);
    let new_width = (child_width * scale_x).max(1.0);
    let new_height = (child_height * scale_y).max(1.0);
    let new_x = new_center_x - new_width / 2.0;
    let new_y = new_center_y - new_height / 2.0;
    let new_rotation = normalize_rotation(child_rotation + delta_rotation);

    (new_x, new_y, new_width, new_height, new_rotation)
}

fn set_element_geometry(
    element: &mut UiElement,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    rotation: f32,
) {
    element
        .properties
        .insert("x".to_string(), serde_json::json!(x));
    element
        .properties
        .insert("y".to_string(), serde_json::json!(y));
    element
        .properties
        .insert("width".to_string(), serde_json::json!(width));
    element
        .properties
        .insert("height".to_string(), serde_json::json!(height));
    element
        .properties
        .insert("rotation".to_string(), serde_json::json!(rotation));
}

fn apply_geometry_snapshot_recursive(
    elements: &mut [UiElement],
    snapshot: &HashMap<Uuid, (f32, f32, f32, f32, f32)>,
    any_updated: &mut bool,
) {
    for element in elements {
        if let Some((x, y, width, height, rotation)) = snapshot.get(&element.id).copied() {
            let previous = CanvasElementData::from_ui_element(element);
            if (previous.x - x).abs() > 0.001
                || (previous.y - y).abs() > 0.001
                || (previous.width - width).abs() > 0.001
                || (previous.height - height).abs() > 0.001
                || (previous.rotation - rotation).abs() > 0.001
            {
                *any_updated = true;
                set_element_geometry(element, x, y, width, height, rotation);
            }
        }

        apply_geometry_snapshot_recursive(&mut element.children, snapshot, any_updated);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProjectSnapshotState {
    project_file: ProjectFile,
    active_page_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorDocumentKind {
    PageUi,
    PageBlueprint,
    ServerBlueprint,
}

pub struct Project {
    project_file: ProjectFile,
    file_path: PathBuf,
    active_page_index: usize,
    assets_root: PathBuf,
}

impl Project {
    /// Create a new project.
    pub fn new(
        name: &str,
        path: &str,
        platform: Platform,
        dev_mode: DevMode,
        initial_page_size: PageSize,
        custom_width: u32,
        custom_height: u32,
    ) -> Self {
        let (width, height) = if initial_page_size == PageSize::Custom {
            (custom_width.max(1), custom_height.max(1))
        } else {
            initial_page_size.default_size()
        };

        let manifest = ProjectManifest {
            project_name: name.to_string(),
            mode: dev_mode.into(),
            platforms: vec![platform.into()],
            ..ProjectManifest::default()
        };

        let mut project_file = ProjectFile::new(manifest);

        let mut main_page = CorePage::new("Main".to_string());
        main_page.children.push(build_page_root(width, height));
        project_file.ui_data.pages.push(main_page);

        let file_path = PathBuf::from(path).join(format!("{name}.spx"));
        let assets_root = std::env::temp_dir().join(format!("snappix-assets-{}", Uuid::new_v4()));

        let mut project = Self {
            project_file,
            file_path,
            active_page_index: 0,
            assets_root,
        };
        project.sync_document_model();
        project
    }

    pub fn name(&self) -> &str {
        &self.project_file.manifest.project_name
    }

    pub fn active_page_index(&self) -> usize {
        self.active_page_index
    }

    pub fn set_active_page(&mut self, index: usize) {
        if index < self.project_file.ui_data.pages.len() {
            self.active_page_index = index;
            if let Some(page) = self.project_file.ui_data.pages.get(index) {
                if matches!(
                    self.project_file.workspace_data.active_document,
                    Some(EditorDocumentRef::PageUi { .. })
                ) {
                    self.project_file.workspace_data.active_document =
                        Some(EditorDocumentRef::PageUi { page_id: page.id });
                }
            }
        }
    }

    pub fn add_page(&mut self, name: &str, width: u32, height: u32) -> usize {
        let mut page = CorePage::new(name.to_string());
        page.children
            .push(build_page_root(width.max(1), height.max(1)));
        let page_id = page.id;
        self.project_file.ui_data.pages.push(page);

        self.active_page_index = self.project_file.ui_data.pages.len() - 1;
        self.sync_document_model();
        let _ = self.open_document(EditorDocumentRef::PageUi { page_id });
        self.active_page_index
    }

    pub fn remove_page(&mut self, index: usize) -> bool {
        if self.project_file.ui_data.pages.len() <= 1
            || index >= self.project_file.ui_data.pages.len()
        {
            return false;
        }

        self.project_file.ui_data.pages.remove(index);

        if self.active_page_index >= self.project_file.ui_data.pages.len() {
            self.active_page_index = self.project_file.ui_data.pages.len() - 1;
        } else if index < self.active_page_index {
            self.active_page_index = self.active_page_index.saturating_sub(1);
        }

        self.sync_document_model();
        true
    }

    pub fn page_names(&self) -> Vec<String> {
        self.project_file
            .ui_data
            .pages
            .iter()
            .map(|p| p.name.clone())
            .collect()
    }

    pub fn page_count(&self) -> usize {
        self.project_file.ui_data.pages.len()
    }

    pub fn rename_page(&mut self, index: usize, new_name: &str) -> bool {
        let normalized = new_name.trim();
        if normalized.is_empty() {
            return false;
        }

        if self.project_file.ui_data.pages.get(index).is_none() {
            return false;
        }

        let mut candidate = normalized.to_string();
        let existing: HashSet<String> = self
            .project_file
            .ui_data
            .pages
            .iter()
            .enumerate()
            .filter_map(|(idx, p)| (idx != index).then_some(p.name.to_lowercase()))
            .collect();
        if existing.contains(&candidate.to_lowercase()) {
            let mut suffix = 1;
            loop {
                let next = format!("{normalized} {suffix}");
                if !existing.contains(&next.to_lowercase()) {
                    candidate = next;
                    break;
                }
                suffix += 1;
            }
        }

        let Some(page) = self.project_file.ui_data.pages.get_mut(index) else {
            return false;
        };
        page.name = candidate;
        self.sync_document_model();
        true
    }

    pub fn page_size(&self, index: usize) -> (u32, u32) {
        self.project_file
            .ui_data
            .pages
            .get(index)
            .map(page_size)
            .unwrap_or((1920, 1080))
    }

    pub fn active_page_size(&self) -> (u32, u32) {
        self.page_size(self.active_page_index)
    }

    pub fn open_documents(&self) -> &[EditorDocumentRef] {
        &self.project_file.workspace_data.open_documents
    }

    pub fn active_document(&self) -> Option<&EditorDocumentRef> {
        self.project_file.workspace_data.active_document.as_ref()
    }

    pub fn active_document_kind(&self) -> Option<EditorDocumentKind> {
        self.active_document()
            .and_then(|document| self.document_kind(document))
    }

    pub fn page_document_ref(&self, page_index: usize) -> Option<EditorDocumentRef> {
        let page = self.project_file.ui_data.pages.get(page_index)?;
        Some(EditorDocumentRef::PageUi { page_id: page.id })
    }

    pub fn page_blueprint_document_ref(&self, page_index: usize) -> Option<EditorDocumentRef> {
        let page = self.project_file.ui_data.pages.get(page_index)?;
        let document = self.page_blueprint_document(page.id)?;
        Some(EditorDocumentRef::PageBlueprint {
            document_id: document.id,
        })
    }

    pub fn server_blueprint_document_ref(&self) -> Option<EditorDocumentRef> {
        let document = self.server_blueprint_document()?;
        Some(EditorDocumentRef::ServerBlueprint {
            document_id: document.id,
        })
    }

    pub fn document_kind(&self, document: &EditorDocumentRef) -> Option<EditorDocumentKind> {
        match document {
            EditorDocumentRef::PageUi { page_id } => self
                .page_index_by_id(*page_id)
                .map(|_| EditorDocumentKind::PageUi),
            EditorDocumentRef::PageBlueprint { document_id } => self
                .blueprint_document(*document_id)
                .map(|_| EditorDocumentKind::PageBlueprint),
            EditorDocumentRef::ServerBlueprint { document_id } => self
                .blueprint_document(*document_id)
                .map(|_| EditorDocumentKind::ServerBlueprint),
        }
    }

    pub fn document_label(&self, document: &EditorDocumentRef) -> Option<String> {
        match document {
            EditorDocumentRef::PageUi { page_id } => {
                self.page_by_id(*page_id).map(|page| page.name.clone())
            }
            EditorDocumentRef::PageBlueprint { document_id }
            | EditorDocumentRef::ServerBlueprint { document_id } => self
                .blueprint_document(*document_id)
                .map(|document| document.name.clone()),
        }
    }

    pub fn linked_page_index_for_document(&self, document: &EditorDocumentRef) -> Option<usize> {
        match document {
            EditorDocumentRef::PageUi { page_id } => self.page_index_by_id(*page_id),
            EditorDocumentRef::PageBlueprint { document_id } => {
                let document = self.blueprint_document(*document_id)?;
                match document.owner {
                    BlueprintOwner::Page { page_id } => self.page_index_by_id(page_id),
                    BlueprintOwner::Project => None,
                }
            }
            EditorDocumentRef::ServerBlueprint { .. } => None,
        }
    }

    pub fn open_document(&mut self, document: EditorDocumentRef) -> bool {
        if !self.document_exists(&document) {
            return false;
        }

        if !self
            .project_file
            .workspace_data
            .open_documents
            .contains(&document)
        {
            self.project_file
                .workspace_data
                .open_documents
                .push(document.clone());
        }
        self.project_file.workspace_data.active_document = Some(document.clone());
        self.align_active_page_with_document(&document);
        self.sync_document_model();
        true
    }

    pub fn close_document(&mut self, tab_index: usize) -> bool {
        if self.project_file.workspace_data.open_documents.len() <= 1
            || tab_index >= self.project_file.workspace_data.open_documents.len()
        {
            return false;
        }

        let removed = self
            .project_file
            .workspace_data
            .open_documents
            .remove(tab_index);
        let next_active = if self
            .project_file
            .workspace_data
            .active_document
            .as_ref()
            .map(|active| active == &removed)
            .unwrap_or(false)
        {
            let fallback_index = tab_index
                .saturating_sub(1)
                .min(self.project_file.workspace_data.open_documents.len() - 1);
            self.project_file
                .workspace_data
                .open_documents
                .get(fallback_index)
                .cloned()
        } else {
            self.project_file.workspace_data.active_document.clone()
        };

        self.project_file.workspace_data.active_document = next_active.clone();
        if let Some(active) = next_active {
            self.align_active_page_with_document(&active);
        }
        self.sync_document_model();
        true
    }

    pub fn select_open_document(&mut self, tab_index: usize) -> bool {
        let Some(document) = self
            .project_file
            .workspace_data
            .open_documents
            .get(tab_index)
            .cloned()
        else {
            return false;
        };

        self.project_file.workspace_data.active_document = Some(document.clone());
        self.align_active_page_with_document(&document);
        self.sync_document_model();
        true
    }

    pub fn build_blueprint_api(&self) -> BlueprintProjectApi {
        let mut logic_documents = self.project_file.logic_data.documents.clone();
        for document in &mut logic_documents {
            document.sync_exports();
        }

        let pages = self
            .project_file
            .ui_data
            .pages
            .iter()
            .map(|page| PageApiDescriptor {
                page_id: page.id,
                page_name: page.name.clone(),
                elements: page
                    .children
                    .first()
                    .map(|root| {
                        let mut flat = Vec::new();
                        collect_elements_recursive(&root.children, &mut flat);
                        flat.into_iter()
                            .map(|element| {
                                let element_type = element_type_name(element);
                                UiElementApiDescriptor {
                                    element_id: element.id,
                                    display_name: element_display_name(element),
                                    element_type: element_type.clone(),
                                    events: blueprint_events_for_element_type(&element_type),
                                    actions: blueprint_actions_for_element_type(&element_type),
                                }
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
                exported_functions: logic_documents
                    .iter()
                    .find(|document| {
                        matches!(
                            document.owner,
                            BlueprintOwner::Page { page_id } if page_id == page.id
                        )
                    })
                    .map(|document| document.exports.clone())
                    .unwrap_or_default(),
            })
            .collect();

        let server = ServerApiDescriptor {
            exported_functions: logic_documents
                .iter()
                .find(|document| document.kind == BlueprintDocumentKind::ServerBlueprint)
                .map(|document| document.exports.clone())
                .unwrap_or_default(),
        };

        BlueprintProjectApi { pages, server }
    }

    pub fn active_blueprint_nodes(&self) -> Vec<BlueprintNode> {
        self.active_blueprint_document()
            .and_then(|document| document.graphs.first())
            .map(|graph| graph.nodes.clone())
            .unwrap_or_default()
    }

    pub fn active_blueprint_local_variables(&self) -> Vec<BlueprintLocalVariable> {
        self.active_blueprint_document()
            .and_then(|document| document.graphs.first())
            .map(|graph| graph.local_variables.clone())
            .unwrap_or_default()
    }

    pub fn active_blueprint_functions(&self) -> Vec<BlueprintFunctionSignature> {
        self.active_blueprint_document()
            .map(|document| {
                document
                    .graphs
                    .iter()
                    .filter_map(BlueprintGraph::function_signature)
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn add_variable_node_to_active_blueprint(
        &mut self,
        variable_id: Uuid,
        access_kind: &str,
        preferred_x: f32,
        preferred_y: f32,
    ) -> Option<Uuid> {
        let graph = self.active_blueprint_graph_mut()?;
        let variable = graph
            .local_variables
            .iter()
            .find(|variable| variable.id == variable_id)
            .cloned()?;

        let mut node = match access_kind {
            "set" | "setter" => BlueprintNode::variable_set(&variable),
            _ => BlueprintNode::variable_get(&variable),
        };
        node.position = nearest_free_blueprint_position(
            &graph.nodes,
            BlueprintPoint {
                x: preferred_x.round() as i32,
                y: preferred_y.round() as i32,
            },
            blueprint_node_size(&node.kind),
        );
        let node_id = node.id;
        graph.nodes.push(node);
        Some(node_id)
    }

    pub fn add_local_variable_to_active_blueprint(&mut self) -> Option<Uuid> {
        let graph = self.active_blueprint_graph_mut()?;
        let next_index = graph.local_variables.len() + 1;
        let variable = BlueprintLocalVariable {
            id: Uuid::new_v4(),
            name: format!("var{next_index}"),
            data_type: BlueprintPinType::Bool,
        };
        let variable_id = variable.id;
        graph.local_variables.push(variable);
        Some(variable_id)
    }

    pub fn rename_local_variable_in_active_blueprint(
        &mut self,
        variable_id: Uuid,
        new_name: &str,
    ) -> bool {
        let name = sanitize_blueprint_symbol(new_name);
        if name.is_empty() {
            return false;
        }

        let Some(graph) = self.active_blueprint_graph_mut() else {
            return false;
        };
        if graph
            .local_variables
            .iter()
            .any(|variable| variable.id != variable_id && variable.name == name)
        {
            return false;
        }

        let Some(variable) = graph
            .local_variables
            .iter_mut()
            .find(|variable| variable.id == variable_id)
        else {
            return false;
        };
        variable.name = name.clone();

        for node in &mut graph.nodes {
            match node.kind {
                BlueprintNodeKind::VariableGet {
                    variable_id: node_variable_id,
                } if node_variable_id == variable_id => {
                    node.title = name.clone();
                }
                BlueprintNodeKind::VariableSet {
                    variable_id: node_variable_id,
                } if node_variable_id == variable_id => {
                    node.title = format!("Set {name}");
                }
                _ => {}
            }
        }

        true
    }

    pub fn set_local_variable_type_in_active_blueprint(
        &mut self,
        variable_id: Uuid,
        type_name: &str,
    ) -> bool {
        let Some(data_type) = parse_blueprint_pin_type_name(type_name) else {
            return false;
        };
        let Some(graph) = self.active_blueprint_graph_mut() else {
            return false;
        };
        let Some(variable) = graph
            .local_variables
            .iter_mut()
            .find(|variable| variable.id == variable_id)
        else {
            return false;
        };
        variable.data_type = data_type;

        for node in &mut graph.nodes {
            match node.kind {
                BlueprintNodeKind::VariableGet {
                    variable_id: node_variable_id,
                }
                | BlueprintNodeKind::VariableSet {
                    variable_id: node_variable_id,
                } if node_variable_id == variable_id => {
                    for pin in &mut node.pins {
                        if pin.name == "value" {
                            pin.data_type = data_type;
                        }
                    }
                }
                _ => {}
            }
        }

        true
    }

    pub fn add_function_to_active_blueprint(&mut self) -> Option<Uuid> {
        let graph_count = self.active_blueprint_document()?.graphs.len();
        let function_name = format!("function{}", graph_count);
        let signature = BlueprintFunctionSignature {
            name: function_name.clone(),
            parameters: Vec::new(),
            return_type: BlueprintPinType::Void,
            is_public: true,
        };
        let entry = BlueprintNode::function_entry(signature.clone());
        let mut graph = BlueprintGraph::new(function_name, BlueprintGraphKind::FunctionGraph);
        graph.entrypoints.push(entry.id);
        graph.nodes.push(entry);
        let graph_id = graph.id;
        let document = self.active_blueprint_document_mut()?;
        document.graphs.push(graph);
        document.sync_exports();
        Some(graph_id)
    }

    pub fn ensure_element_event_nodes_on_active_page_blueprint(
        &mut self,
        element_id: Uuid,
    ) -> bool {
        let Some(element) = self.get_element_on_active_page(element_id) else {
            return false;
        };
        let events = blueprint_events_for_element_type(&element.element_type);
        if events.is_empty() {
            return false;
        }

        let Some(document_ref) = self.page_blueprint_document_ref(self.active_page_index()) else {
            return false;
        };
        if !self.open_document(document_ref) {
            return false;
        }

        let Some(graph) = self.active_blueprint_graph_mut() else {
            return false;
        };
        let mut changed = false;
        for (idx, event) in events.iter().enumerate() {
            let exists = graph.nodes.iter().any(|node| {
                matches!(
                    &node.kind,
                    core_blueprint::BlueprintNodeKind::UiEvent {
                        element_id: node_element_id,
                        event_name
                    } if *node_element_id == element_id && event_name == &event.name
                )
            });
            if exists {
                continue;
            }

            let mut node = BlueprintNode::ui_event(element_id, event.name.clone());
            node.title = format!("{} {}", element.element_type, event.display_name);
            node.position = BlueprintPoint {
                x: 80,
                y: 80 + idx as i32 * 120,
            };
            graph.entrypoints.push(node.id);
            graph.nodes.push(node);
            changed = true;
        }
        changed
    }

    pub fn compile_blueprints(&mut self) -> BlueprintCompilationResult {
        self.sync_document_model();
        let api = self.build_blueprint_api();
        let build_dir = self
            .file_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(".snappix")
            .join("build");
        compile_project(
            self.name(),
            &self.project_file.logic_data.documents,
            &api,
            build_dir,
        )
    }

    fn page_by_id(&self, page_id: Uuid) -> Option<&CorePage> {
        self.project_file
            .ui_data
            .pages
            .iter()
            .find(|page| page.id == page_id)
    }

    fn page_index_by_id(&self, page_id: Uuid) -> Option<usize> {
        self.project_file
            .ui_data
            .pages
            .iter()
            .position(|page| page.id == page_id)
    }

    fn blueprint_document(&self, document_id: Uuid) -> Option<&BlueprintDocument> {
        self.project_file
            .logic_data
            .documents
            .iter()
            .find(|document| document.id == document_id)
    }

    fn active_blueprint_document(&self) -> Option<&BlueprintDocument> {
        let document_id = match self.active_document()? {
            EditorDocumentRef::PageBlueprint { document_id }
            | EditorDocumentRef::ServerBlueprint { document_id } => *document_id,
            EditorDocumentRef::PageUi { .. } => return None,
        };
        self.blueprint_document(document_id)
    }

    fn active_blueprint_document_mut(&mut self) -> Option<&mut BlueprintDocument> {
        let document_id = match self.project_file.workspace_data.active_document.as_ref()? {
            EditorDocumentRef::PageBlueprint { document_id }
            | EditorDocumentRef::ServerBlueprint { document_id } => *document_id,
            EditorDocumentRef::PageUi { .. } => return None,
        };
        self.project_file
            .logic_data
            .documents
            .iter_mut()
            .find(|document| document.id == document_id)
    }

    fn active_blueprint_graph_mut(&mut self) -> Option<&mut BlueprintGraph> {
        let document = self.active_blueprint_document_mut()?;
        if document.graphs.is_empty() {
            document.graphs.push(BlueprintGraph::new(
                "Events",
                BlueprintGraphKind::EventGraph,
            ));
        }
        document.graphs.first_mut()
    }

    fn page_blueprint_document(&self, page_id: Uuid) -> Option<&BlueprintDocument> {
        self.project_file.logic_data.documents.iter().find(|document| {
            document.kind == BlueprintDocumentKind::PageBlueprint
                && matches!(document.owner, BlueprintOwner::Page { page_id: owner_id } if owner_id == page_id)
        })
    }

    fn server_blueprint_document(&self) -> Option<&BlueprintDocument> {
        self.project_file
            .logic_data
            .documents
            .iter()
            .find(|document| document.kind == BlueprintDocumentKind::ServerBlueprint)
    }

    fn document_exists(&self, document: &EditorDocumentRef) -> bool {
        match document {
            EditorDocumentRef::PageUi { page_id } => self.page_by_id(*page_id).is_some(),
            EditorDocumentRef::PageBlueprint { document_id }
            | EditorDocumentRef::ServerBlueprint { document_id } => {
                self.blueprint_document(*document_id).is_some()
            }
        }
    }

    fn align_active_page_with_document(&mut self, document: &EditorDocumentRef) {
        if let Some(page_index) = self.linked_page_index_for_document(document) {
            self.active_page_index = page_index;
        }
    }

    fn sync_document_model(&mut self) {
        let pages: Vec<(Uuid, String)> = self
            .project_file
            .ui_data
            .pages
            .iter()
            .map(|page| (page.id, page.name.clone()))
            .collect();
        let mut current_documents = std::mem::take(&mut self.project_file.logic_data.documents);
        let mut next_documents = Vec::new();

        for (page_id, page_name) in pages {
            if let Some(position) = current_documents.iter().position(|document| {
                document.kind == BlueprintDocumentKind::PageBlueprint
                    && matches!(document.owner, BlueprintOwner::Page { page_id: owner_id } if owner_id == page_id)
            }) {
                let mut document = current_documents.remove(position);
                document.kind = BlueprintDocumentKind::PageBlueprint;
                document.owner = BlueprintOwner::Page { page_id };
                document.name = blueprint_name_for_page(&page_name);
                document.sync_exports();
                next_documents.push(document);
            } else {
                let mut document = BlueprintDocument::new_page(page_id, &page_name);
                document.sync_exports();
                next_documents.push(document);
            }
        }

        if let Some(position) = current_documents
            .iter()
            .position(|document| document.kind == BlueprintDocumentKind::ServerBlueprint)
        {
            let mut document = current_documents.remove(position);
            document.kind = BlueprintDocumentKind::ServerBlueprint;
            document.owner = BlueprintOwner::Project;
            document.name = "server.blp".to_string();
            document.sync_exports();
            next_documents.push(document);
        } else {
            let mut document = BlueprintDocument::new_server();
            document.sync_exports();
            next_documents.push(document);
        }

        self.project_file.logic_data.documents = next_documents;
        let valid_page_ids: HashSet<Uuid> = self
            .project_file
            .ui_data
            .pages
            .iter()
            .map(|page| page.id)
            .collect();
        let valid_blueprint_ids: HashSet<Uuid> = self
            .project_file
            .logic_data
            .documents
            .iter()
            .map(|document| document.id)
            .collect();

        self.project_file
            .workspace_data
            .open_documents
            .retain(|document| match document {
                EditorDocumentRef::PageUi { page_id } => valid_page_ids.contains(page_id),
                EditorDocumentRef::PageBlueprint { document_id }
                | EditorDocumentRef::ServerBlueprint { document_id } => {
                    valid_blueprint_ids.contains(document_id)
                }
            });
        self.project_file.workspace_data.active_document = self
            .project_file
            .workspace_data
            .active_document
            .clone()
            .filter(|document| match document {
                EditorDocumentRef::PageUi { page_id } => valid_page_ids.contains(page_id),
                EditorDocumentRef::PageBlueprint { document_id }
                | EditorDocumentRef::ServerBlueprint { document_id } => {
                    valid_blueprint_ids.contains(document_id)
                }
            });

        if self.project_file.ui_data.pages.is_empty() {
            self.project_file.workspace_data.open_documents.clear();
            self.project_file.workspace_data.active_document = None;
            self.active_page_index = 0;
            return;
        }

        self.active_page_index = self
            .active_page_index
            .min(self.project_file.ui_data.pages.len().saturating_sub(1));

        if self.project_file.workspace_data.open_documents.is_empty() {
            let page_id = self.project_file.ui_data.pages[self.active_page_index].id;
            self.project_file
                .workspace_data
                .open_documents
                .push(EditorDocumentRef::PageUi { page_id });
        }

        if self.project_file.workspace_data.active_document.is_none() {
            self.project_file.workspace_data.active_document = self
                .project_file
                .workspace_data
                .open_documents
                .first()
                .cloned();
        }

        if let Some(active) = self.project_file.workspace_data.active_document.clone() {
            if !self
                .project_file
                .workspace_data
                .open_documents
                .contains(&active)
            {
                self.project_file
                    .workspace_data
                    .open_documents
                    .push(active.clone());
            }
            self.align_active_page_with_document(&active);
        }
    }

    fn active_page_root(&self) -> Option<&UiElement> {
        self.project_file
            .ui_data
            .pages
            .get(self.active_page_index)?
            .children
            .first()
    }

    fn active_page_root_mut(&mut self) -> Option<&mut UiElement> {
        self.project_file
            .ui_data
            .pages
            .get_mut(self.active_page_index)?
            .children
            .first_mut()
    }

    fn active_page(&self) -> Option<&CorePage> {
        self.project_file.ui_data.pages.get(self.active_page_index)
    }

    fn active_page_mut(&mut self) -> Option<&mut CorePage> {
        self.project_file
            .ui_data
            .pages
            .get_mut(self.active_page_index)
    }

    fn project_root_dir(&self) -> PathBuf {
        self.assets_root.clone()
    }

    fn referenced_image_paths(&self) -> HashSet<String> {
        let mut paths = HashSet::new();
        for page in &self.project_file.ui_data.pages {
            collect_image_paths_from_elements(&page.children, &mut paths);
            for comment in &page.comments {
                if let Some(image) = &comment.image {
                    if !image.path.trim().is_empty() {
                        let normalized = normalize_asset_path(&image.path);
                        if is_project_image_asset(&normalized) {
                            paths.insert(normalized);
                        }
                    }
                }
            }
        }
        paths
    }

    fn prune_unused_image_assets(&self, candidates: &[String]) {
        if candidates.is_empty() {
            return;
        }

        let referenced = self.referenced_image_paths();
        let root = self.project_root_dir();

        for candidate in candidates {
            let normalized = normalize_asset_path(candidate);
            if normalized.is_empty() || referenced.contains(&normalized) {
                continue;
            }
            if !is_project_image_asset(&normalized) {
                continue;
            }
            let full_path = root.join(normalized);
            let _ = std::fs::remove_file(full_path);
        }
    }

    pub fn active_page_elements(&self) -> Vec<CanvasElementData> {
        self.active_page_root()
            .map(|root| {
                let mut flat = Vec::new();
                collect_elements_recursive(&root.children, &mut flat);
                flat.into_iter()
                    .map(CanvasElementData::from_ui_element)
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn active_page_comments(&self) -> Vec<CorePageComment> {
        self.active_page()
            .map(|page| page.comments.clone())
            .unwrap_or_default()
    }

    pub fn add_comment_to_active_page(&mut self, x: f32, y: f32) -> Option<Uuid> {
        let page = self.active_page_mut()?;
        let comment = CorePageComment::new(x, y);
        let comment_id = comment.id;
        page.comments.push(comment);
        Some(comment_id)
    }

    pub fn update_comment_content_on_active_page(
        &mut self,
        comment_id: Uuid,
        title: &str,
        body: &str,
    ) -> bool {
        let Some(comment) = self.active_page_mut().and_then(|page| {
            page.comments
                .iter_mut()
                .find(|comment| comment.id == comment_id)
        }) else {
            return false;
        };

        comment.title = truncate_chars(title, COMMENT_TITLE_MAX_CHARS)
            .trim()
            .to_string();
        if comment.title.is_empty() {
            comment.title = "Comment".to_string();
        }
        comment.body = truncate_chars(body, COMMENT_BODY_MAX_CHARS);
        true
    }

    pub fn update_comment_position_on_active_page(
        &mut self,
        comment_id: Uuid,
        x: f32,
        y: f32,
    ) -> bool {
        let Some(comment) = self.active_page_mut().and_then(|page| {
            page.comments
                .iter_mut()
                .find(|comment| comment.id == comment_id)
        }) else {
            return false;
        };

        if !(x.is_finite() && y.is_finite()) {
            return false;
        }

        comment.x = x;
        comment.y = y;
        true
    }

    pub fn update_comment_font_sizes_on_active_page(
        &mut self,
        comment_id: Uuid,
        title_font_size: f32,
        body_font_size: f32,
    ) -> bool {
        let Some(comment) = self.active_page_mut().and_then(|page| {
            page.comments
                .iter_mut()
                .find(|comment| comment.id == comment_id)
        }) else {
            return false;
        };

        if !(title_font_size.is_finite() && body_font_size.is_finite()) {
            return false;
        }

        comment.title_font_size = title_font_size.clamp(10.0, 36.0);
        comment.body_font_size = body_font_size.clamp(10.0, 30.0);
        true
    }

    pub fn update_comment_size_on_active_page(
        &mut self,
        comment_id: Uuid,
        width: f32,
        body_height: f32,
    ) -> bool {
        let Some(comment) = self.active_page_mut().and_then(|page| {
            page.comments
                .iter_mut()
                .find(|comment| comment.id == comment_id)
        }) else {
            return false;
        };

        if !(width.is_finite() && body_height.is_finite()) {
            return false;
        }

        comment.width = width.clamp(220.0, 720.0);
        comment.body_height = body_height.clamp(76.0, 520.0);
        true
    }

    pub fn set_comment_image_on_active_page(
        &mut self,
        comment_id: Uuid,
        image_path: &str,
        width: u32,
        height: u32,
    ) -> bool {
        let normalized_path = normalized_image_asset_path(image_path, false);
        if width == 0 || height == 0 || normalized_path.is_empty() {
            return false;
        }

        let Some(comment) = self.active_page_mut().and_then(|page| {
            page.comments
                .iter_mut()
                .find(|comment| comment.id == comment_id)
        }) else {
            return false;
        };

        let previous_path = comment.image.as_ref().map(|image| image.path.clone());

        let new_path = normalized_path.clone();
        comment.image = Some(CorePageCommentImage {
            width,
            height,
            path: new_path,
            rgba: Vec::new(),
        });
        if let Some(previous) = previous_path {
            if previous != normalized_path {
                self.prune_unused_image_assets(&[previous]);
            }
        }
        true
    }

    pub fn clear_comment_image_on_active_page(&mut self, comment_id: Uuid) -> bool {
        let Some(comment) = self.active_page_mut().and_then(|page| {
            page.comments
                .iter_mut()
                .find(|comment| comment.id == comment_id)
        }) else {
            return false;
        };

        if comment.image.is_none() {
            return false;
        }

        let previous_path = comment.image.as_ref().map(|image| image.path.clone());
        comment.image = None;
        if let Some(previous) = previous_path {
            self.prune_unused_image_assets(&[previous]);
        }
        true
    }

    pub fn remove_comment_on_active_page(&mut self, comment_id: Uuid) -> bool {
        let Some(page) = self.active_page_mut() else {
            return false;
        };

        let removed_image = page
            .comments
            .iter()
            .find(|comment| comment.id == comment_id)
            .and_then(|comment| comment.image.as_ref().map(|image| image.path.clone()));

        let before = page.comments.len();
        page.comments.retain(|comment| comment.id != comment_id);
        let removed = before != page.comments.len();
        if removed {
            if let Some(previous) = removed_image {
                self.prune_unused_image_assets(&[previous]);
            }
        }
        removed
    }

    pub fn snapshot_element_geometries_on_active_page(
        &self,
    ) -> HashMap<Uuid, (f32, f32, f32, f32, f32)> {
        self.active_page_elements()
            .into_iter()
            .map(|element| {
                (
                    element.id,
                    (
                        element.x,
                        element.y,
                        element.width,
                        element.height,
                        element.rotation,
                    ),
                )
            })
            .collect()
    }

    pub fn restore_element_geometries_on_active_page(
        &mut self,
        snapshot: &HashMap<Uuid, (f32, f32, f32, f32, f32)>,
    ) -> bool {
        if snapshot.is_empty() {
            return false;
        }

        let Some(root) = self.active_page_root_mut() else {
            return false;
        };

        let mut any_updated = false;
        apply_geometry_snapshot_recursive(&mut root.children, snapshot, &mut any_updated);
        any_updated
    }

    fn element_name_set_on_active_page(&self, except_id: Option<Uuid>) -> HashSet<String> {
        let mut names = HashSet::new();
        if let Some(root) = self.active_page_root() {
            let mut flat = Vec::new();
            collect_elements_recursive(&root.children, &mut flat);
            for element in flat {
                if except_id.is_some() && except_id == Some(element.id) {
                    continue;
                }
                let name = get_string_property(element, "name")
                    .or_else(|| get_string_property(element, "display_name"))
                    .unwrap_or_else(|| {
                        get_string_property(element, "element_type")
                            .unwrap_or_else(|| "component".to_string())
                    })
                    .to_lowercase();
                names.insert(name);
            }
        }
        names
    }

    fn unique_element_name_on_active_page(
        &self,
        preferred_name: &str,
        fallback_type: &str,
        except_id: Option<Uuid>,
    ) -> String {
        let normalized = preferred_name.trim().to_lowercase();
        let base = if normalized.is_empty() {
            CanvasElementData::name_seed(fallback_type)
        } else {
            normalized
        };

        let used = self.element_name_set_on_active_page(except_id);
        if !used.contains(&base) {
            return base;
        }

        let mut i = 1;
        loop {
            let candidate = format!("{base}{i}");
            if !used.contains(&candidate) {
                return candidate;
            }
            i += 1;
        }
    }

    fn is_layout_container_type(element_type: &str) -> bool {
        matches!(
            element_type,
            "div"
                | "stack-container"
                | "stack"
                | "flex-container"
                | "flex"
                | "grid-container"
                | "grid"
        )
    }

    fn layout_children_in_parent_on_active_page(&mut self, parent_id: Uuid) -> bool {
        let Some(parent) = self.get_element_on_active_page(parent_id) else {
            return false;
        };
        if !Self::is_layout_container_type(parent.element_type.as_str()) {
            return false;
        }

        let container_mode = effective_container_mode_for_canvas(&parent);
        if !container_mode.is_managed() {
            return false;
        }

        let all_elements = self.active_page_elements();
        let order_map: HashMap<Uuid, usize> = all_elements
            .iter()
            .enumerate()
            .map(|(idx, element)| (element.id, idx))
            .collect();
        let mut children: Vec<CanvasElementData> = all_elements
            .into_iter()
            .filter(|element| parse_parent_property_from_canvas(element) == Some(parent_id))
            .collect();
        if children.is_empty() {
            return false;
        }

        children.sort_by(|left, right| {
            let left_order = canvas_prop_f32(left, "layout_order", 0.0);
            let right_order = canvas_prop_f32(right, "layout_order", 0.0);
            left_order
                .partial_cmp(&right_order)
                .unwrap_or(Ordering::Equal)
                .then_with(|| {
                    order_map
                        .get(&left.id)
                        .copied()
                        .unwrap_or(usize::MAX)
                        .cmp(&order_map.get(&right.id).copied().unwrap_or(usize::MAX))
                })
        });

        let allows_absolute_children = container_allows_absolute_children(&parent);
        let mut absolute_children = Vec::new();
        let mut flow_children = Vec::new();
        for child in children {
            if allows_absolute_children && child_uses_absolute_positioning(&child) {
                absolute_children.push(child);
            } else {
                flow_children.push(child);
            }
        }

        let padding = LayoutEdges::from_canvas(&parent, "layout_padding", 8.0);
        let gap = canvas_prop_f32(&parent, "layout_spacing", 8.0).max(0.0);
        let content_x = parent.x + padding.left;
        let content_y = parent.y + padding.top;
        let content_w = (parent.width - padding.left - padding.right).max(1.0);
        let content_h = (parent.height - padding.top - padding.bottom).max(1.0);

        let mut changed = false;
        for child in absolute_children {
            changed |= self.update_element_geometry_on_active_page(
                child.id,
                child.x,
                child.y,
                child.width,
                child.height,
                child.rotation,
            );
            if effective_container_mode_for_canvas(&child).is_managed() {
                changed |= self.layout_children_in_parent_on_active_page(child.id);
            }
        }
        if flow_children.is_empty() {
            return changed;
        }

        match container_mode {
            ContainerMode::Stack => {
                let stack_alignment = normalize_stack_alignment(&canvas_prop_str(
                    &parent,
                    "stack_alignment",
                    "stretch",
                ));
                let mut cursor_y = content_y;
                for child in flow_children {
                    let margin = LayoutEdges::from_canvas(&child, "layout_margin", 0.0);
                    let available_w = (content_w - margin.horizontal()).max(1.0);
                    let remaining_h =
                        (content_y + content_h - cursor_y - margin.vertical()).max(1.0);
                    let child_h = child.height.max(1.0).min(remaining_h);
                    let child_w = if stack_alignment == "stretch" {
                        available_w
                    } else {
                        child.width.max(1.0).min(available_w)
                    };
                    let child_x = match stack_alignment.as_str() {
                        "center" => content_x + (content_w - child_w) / 2.0,
                        "end" => content_x + content_w - child_w - margin.right,
                        _ => content_x + margin.left,
                    };
                    cursor_y += margin.top;
                    changed |= self.update_element_geometry_on_active_page(
                        child.id,
                        child_x,
                        cursor_y,
                        child_w,
                        child_h,
                        child.rotation,
                    );
                    if effective_container_mode_for_canvas(&child).is_managed() {
                        changed |= self.layout_children_in_parent_on_active_page(child.id);
                    }
                    cursor_y += child_h + margin.bottom + gap;
                }
                changed
            }
            ContainerMode::Flex => {
                #[derive(Clone)]
                struct FlexLine {
                    items: Vec<CanvasElementData>,
                    main_size: f32,
                    cross_size: f32,
                }

                let flex_direction =
                    normalize_flex_direction(&canvas_prop_str(&parent, "flex_direction", "column"));
                let flex_wrap =
                    normalize_flex_wrap(&canvas_prop_str(&parent, "flex_wrap", "nowrap"));
                let justify_content = normalize_justify_content(&canvas_prop_str(
                    &parent,
                    "justify_content",
                    "flex-start",
                ));
                let mut align_items =
                    normalize_item_alignment(&canvas_prop_str(&parent, "align_items", "stretch"));
                let align_content =
                    normalize_align_content(&canvas_prop_str(&parent, "align_content", "stretch"));
                let place_items_raw = canvas_prop_str(&parent, "place_items", "");
                if !place_items_raw.trim().is_empty() {
                    let (_, place_align) =
                        parse_place_items(&place_items_raw, "stretch", align_items.as_str());
                    align_items = place_align;
                }

                let wrap_enabled = flex_wrap != "nowrap";
                let mut lines = Vec::new();
                let mut current_line = FlexLine {
                    items: Vec::new(),
                    main_size: 0.0,
                    cross_size: 0.0,
                };

                for child in flow_children {
                    let margin = LayoutEdges::from_canvas(&child, "layout_margin", 0.0);
                    let outer_main = if flex_direction == "row" {
                        child.width.max(1.0) + margin.horizontal()
                    } else {
                        child.height.max(1.0) + margin.vertical()
                    };
                    let outer_cross = if flex_direction == "row" {
                        child.height.max(1.0) + margin.vertical()
                    } else {
                        child.width.max(1.0) + margin.horizontal()
                    };
                    let main_limit = if flex_direction == "row" {
                        content_w
                    } else {
                        content_h
                    };
                    let projected_main = if current_line.items.is_empty() {
                        outer_main
                    } else {
                        current_line.main_size + gap + outer_main
                    };

                    if wrap_enabled && !current_line.items.is_empty() && projected_main > main_limit
                    {
                        lines.push(current_line);
                        current_line = FlexLine {
                            items: Vec::new(),
                            main_size: 0.0,
                            cross_size: 0.0,
                        };
                    }

                    current_line.main_size = if current_line.items.is_empty() {
                        outer_main
                    } else {
                        current_line.main_size + gap + outer_main
                    };
                    current_line.cross_size = current_line.cross_size.max(outer_cross);
                    current_line.items.push(child);
                }
                if !current_line.items.is_empty() {
                    lines.push(current_line);
                }
                if lines.is_empty() {
                    return changed;
                }

                if flex_wrap == "wrap-reverse" {
                    lines.reverse();
                }

                let total_cross_before_stretch =
                    lines.iter().map(|line| line.cross_size).sum::<f32>()
                        + gap * lines.len().saturating_sub(1) as f32;
                let available_cross = if flex_direction == "row" {
                    content_h
                } else {
                    content_w
                };
                if wrap_enabled
                    && lines.len() > 1
                    && align_content == "stretch"
                    && total_cross_before_stretch < available_cross
                {
                    let extra = (available_cross - total_cross_before_stretch) / lines.len() as f32;
                    for line in &mut lines {
                        line.cross_size += extra;
                    }
                }

                let total_cross = lines.iter().map(|line| line.cross_size).sum::<f32>()
                    + gap * lines.len().saturating_sub(1) as f32;
                let remaining_cross = (available_cross - total_cross).max(0.0);
                let (line_start, line_gap) = layout_sequence_offsets(
                    if align_content == "stretch" {
                        "flex-start"
                    } else {
                        align_content.as_str()
                    },
                    remaining_cross,
                    lines.len(),
                    gap,
                );

                let mut cursor_cross = if flex_direction == "row" {
                    content_y + line_start
                } else {
                    content_x + line_start
                };

                for line in lines {
                    let main_limit = if flex_direction == "row" {
                        content_w
                    } else {
                        content_h
                    };
                    let remaining_main = (main_limit - line.main_size).max(0.0);
                    let (item_start, item_gap) = layout_sequence_offsets(
                        justify_content.as_str(),
                        remaining_main,
                        line.items.len(),
                        gap,
                    );
                    let mut cursor_main = if flex_direction == "row" {
                        content_x + item_start
                    } else {
                        content_y + item_start
                    };

                    for child in line.items {
                        let margin = LayoutEdges::from_canvas(&child, "layout_margin", 0.0);
                        let (child_x, child_y, child_w, child_h, advance) = if flex_direction
                            == "row"
                        {
                            let used_main = (cursor_main - content_x).max(0.0);
                            let available_main =
                                (main_limit - used_main - margin.horizontal()).max(1.0);
                            let child_w = child.width.max(1.0).min(available_main);
                            let max_cross_size = (line.cross_size - margin.vertical()).max(1.0);
                            let child_h = child.height.max(1.0).min(max_cross_size);
                            let child_y = match align_items.as_str() {
                                "center" => cursor_cross + (line.cross_size - child_h) / 2.0,
                                "flex-end" => {
                                    cursor_cross + line.cross_size - child_h - margin.bottom
                                }
                                _ => cursor_cross + margin.top,
                            };
                            (
                                cursor_main + margin.left,
                                child_y,
                                child_w,
                                child_h,
                                child_w + margin.horizontal() + item_gap,
                            )
                        } else {
                            let used_main = (cursor_main - content_y).max(0.0);
                            let available_main =
                                (main_limit - used_main - margin.vertical()).max(1.0);
                            let child_h = child.height.max(1.0).min(available_main);
                            let max_cross_size = (line.cross_size - margin.horizontal()).max(1.0);
                            let child_w = child.width.max(1.0).min(max_cross_size);
                            let child_x = match align_items.as_str() {
                                "center" => cursor_cross + (line.cross_size - child_w) / 2.0,
                                "flex-end" => {
                                    cursor_cross + line.cross_size - child_w - margin.right
                                }
                                _ => cursor_cross + margin.left,
                            };
                            (
                                child_x,
                                cursor_main + margin.top,
                                child_w,
                                child_h,
                                child_h + margin.vertical() + item_gap,
                            )
                        };

                        changed |= self.update_element_geometry_on_active_page(
                            child.id,
                            child_x,
                            child_y,
                            child_w,
                            child_h,
                            child.rotation,
                        );
                        if effective_container_mode_for_canvas(&child).is_managed() {
                            changed |= self.layout_children_in_parent_on_active_page(child.id);
                        }
                        cursor_main += advance;
                    }

                    cursor_cross += line.cross_size + line_gap;
                }

                changed
            }
            ContainerMode::Grid => {
                let justify_content = normalize_justify_content(&canvas_prop_str(
                    &parent,
                    "justify_content",
                    "flex-start",
                ));
                let align_content = normalize_align_content(&canvas_prop_str(
                    &parent,
                    "align_content",
                    "flex-start",
                ));
                let base_justify_items =
                    normalize_item_alignment(&canvas_prop_str(&parent, "justify_items", "stretch"));
                let base_align_items =
                    normalize_item_alignment(&canvas_prop_str(&parent, "align_items", "stretch"));
                let place_items_raw = canvas_prop_str(&parent, "place_items", "");
                let (justify_items, align_items) = if place_items_raw.trim().is_empty() {
                    (base_justify_items, base_align_items)
                } else {
                    parse_place_items(
                        &place_items_raw,
                        base_justify_items.as_str(),
                        base_align_items.as_str(),
                    )
                };

                let columns_value = {
                    let explicit = canvas_prop_str(&parent, "grid_template_columns", "");
                    if explicit.trim().is_empty() {
                        canvas_prop_str(&parent, "grid_columns", "1fr 1fr")
                    } else {
                        explicit
                    }
                };
                let rows_value = {
                    let explicit = canvas_prop_str(&parent, "grid_template_rows", "");
                    if explicit.trim().is_empty() {
                        canvas_prop_str(&parent, "grid_rows", "auto auto")
                    } else {
                        explicit
                    }
                };

                let column_tracks = parse_grid_tracks(&columns_value, "1fr 1fr");
                let column_count = column_tracks.len().max(1);
                let mut row_tracks = parse_grid_tracks(&rows_value, "auto auto");
                let required_rows = flow_children.len().div_ceil(column_count).max(1);
                if row_tracks.len() < required_rows {
                    row_tracks.resize(required_rows, GridTrack::Auto);
                }

                let mut row_auto_mins = vec![0.0_f32; row_tracks.len()];
                for (idx, child) in flow_children.iter().enumerate() {
                    let row = idx / column_count;
                    if row >= row_auto_mins.len() {
                        break;
                    }
                    let margin = LayoutEdges::from_canvas(child, "layout_margin", 0.0);
                    row_auto_mins[row] =
                        row_auto_mins[row].max(child.height.max(1.0) + margin.vertical());
                }

                let column_widths = resolve_grid_tracks(
                    &column_tracks,
                    content_w,
                    gap,
                    &vec![0.0; column_tracks.len()],
                );
                let row_heights = resolve_grid_tracks(&row_tracks, content_h, gap, &row_auto_mins);
                let total_grid_w = column_widths.iter().sum::<f32>()
                    + gap * column_widths.len().saturating_sub(1) as f32;
                let total_grid_h = row_heights.iter().sum::<f32>()
                    + gap * row_heights.len().saturating_sub(1) as f32;
                let (grid_start_x, grid_gap_x) = layout_sequence_offsets(
                    justify_content.as_str(),
                    (content_w - total_grid_w).max(0.0),
                    column_widths.len(),
                    gap,
                );
                let (grid_start_y, grid_gap_y) = layout_sequence_offsets(
                    if align_content == "stretch" {
                        "flex-start"
                    } else {
                        align_content.as_str()
                    },
                    (content_h - total_grid_h).max(0.0),
                    row_heights.len(),
                    gap,
                );
                let grid_x = content_x + grid_start_x;
                let grid_y = content_y + grid_start_y;

                let mut row_y = grid_y;
                for (row_idx, row_height) in row_heights.iter().enumerate() {
                    let mut column_x = grid_x;
                    for (column_idx, column_width) in column_widths.iter().enumerate() {
                        let child_idx = row_idx * column_count + column_idx;
                        let Some(child) = flow_children.get(child_idx).cloned() else {
                            column_x += *column_width + gap;
                            continue;
                        };

                        let margin = LayoutEdges::from_canvas(&child, "layout_margin", 0.0);
                        let max_w = (*column_width - margin.horizontal()).max(1.0);
                        let max_h = (*row_height - margin.vertical()).max(1.0);
                        let child_w = if justify_items == "stretch" {
                            max_w
                        } else {
                            child.width.max(1.0).min(max_w)
                        };
                        let child_h = if align_items == "stretch" {
                            max_h
                        } else {
                            child.height.max(1.0).min(max_h)
                        };
                        let child_x = match justify_items.as_str() {
                            "center" => column_x + (*column_width - child_w) / 2.0,
                            "flex-end" => column_x + *column_width - child_w - margin.right,
                            _ => column_x + margin.left,
                        };
                        let child_y = match align_items.as_str() {
                            "center" => row_y + (*row_height - child_h) / 2.0,
                            "flex-end" => row_y + *row_height - child_h - margin.bottom,
                            _ => row_y + margin.top,
                        };

                        changed |= self.update_element_geometry_on_active_page(
                            child.id,
                            child_x,
                            child_y,
                            child_w,
                            child_h,
                            child.rotation,
                        );
                        if effective_container_mode_for_canvas(&child).is_managed() {
                            changed |= self.layout_children_in_parent_on_active_page(child.id);
                        }

                        column_x += *column_width + grid_gap_x;
                    }
                    row_y += *row_height + grid_gap_y;
                }

                changed
            }
            ContainerMode::Absolute => changed,
        }
    }

    fn update_element_parent_property(
        &mut self,
        element_id: Uuid,
        parent_id: Option<Uuid>,
    ) -> bool {
        let parent_id = parent_id.filter(|parent| *parent != element_id);
        if let Some(parent) = parent_id {
            let Some(root) = self.active_page_root() else {
                return false;
            };
            if find_element_recursive(&root.children, parent).is_none() {
                return false;
            }

            let mut current = Some(parent);
            while let Some(cursor) = current {
                if cursor == element_id {
                    return false;
                }
                current = self.element_parent_on_active_page(cursor);
            }
        }

        let Some(root) = self.active_page_root_mut() else {
            return false;
        };
        let Some(element) = find_element_recursive_mut(&mut root.children, element_id) else {
            return false;
        };
        if let Some(parent) = parent_id {
            element.properties.insert(
                "parent_id".to_string(),
                serde_json::json!(parent.to_string()),
            );
        } else {
            remove_parent_property(element);
        }
        true
    }

    pub fn add_element_to_active_page_with_parent(
        &mut self,
        element: CanvasElementData,
        parent_id: Option<Uuid>,
    ) -> Option<Uuid> {
        let id = element.id;
        let fallback_type = element.element_type.clone();
        let mut ui_element = element.to_ui_element();

        let preferred_name = ui_element
            .properties
            .get("name")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string();
        let unique_name =
            self.unique_element_name_on_active_page(&preferred_name, &fallback_type, None);
        ui_element
            .properties
            .insert("name".to_string(), serde_json::json!(unique_name));

        if let Some(parent) = parent_id {
            let valid_parent = self
                .active_page_root()
                .and_then(|root| find_element_recursive(&root.children, parent))
                .is_some();
            if valid_parent {
                ui_element.properties.insert(
                    "parent_id".to_string(),
                    serde_json::json!(parent.to_string()),
                );
            }
        }

        {
            let root = self.active_page_root_mut()?;
            root.children.push(ui_element);
        }

        if let Some(parent_id) = parent_id {
            if let Some(added) = self.get_element_on_active_page(id) {
                let _ = self.update_element_geometry_on_active_page(
                    id,
                    added.x,
                    added.y,
                    added.width,
                    added.height,
                    added.rotation,
                );
            }
            let _ = self.layout_children_in_parent_on_active_page(parent_id);
        }

        Some(id)
    }

    pub fn update_element_geometry_on_active_page(
        &mut self,
        element_id: Uuid,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        rotation: f32,
    ) -> bool {
        let all_elements = self.active_page_elements();
        let element_map: HashMap<Uuid, CanvasElementData> = all_elements
            .iter()
            .cloned()
            .map(|element| (element.id, element))
            .collect();

        let Some(current) = element_map.get(&element_id) else {
            return false;
        };
        let current = current.clone();

        let mut parent_map: HashMap<Uuid, Option<Uuid>> = HashMap::new();
        let mut children_map: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
        for element in &all_elements {
            let parent_id = parse_parent_property_from_canvas(element).filter(|parent_id| {
                *parent_id != element.id && element_map.contains_key(parent_id)
            });
            parent_map.insert(element.id, parent_id);
            if let Some(parent_id) = parent_id {
                children_map.entry(parent_id).or_default().push(element.id);
            }
        }

        let parent_bounds = parent_map
            .get(&element_id)
            .and_then(|parent_id| *parent_id)
            .and_then(|parent_id| element_map.get(&parent_id))
            .cloned();

        let mut final_width = width.max(1.0);
        let mut final_height = height.max(1.0);
        let mut final_x = x.max(0.0);
        let mut final_y = y.max(0.0);

        if let Some(parent) = parent_bounds {
            let clamped = clamp_to_parent_bounds(
                final_x,
                final_y,
                final_width,
                final_height,
                rotation,
                parent.x,
                parent.y,
                parent.width,
                parent.height,
                parent.rotation,
            );
            final_x = clamped.0;
            final_y = clamped.1;
            final_width = clamped.2;
            final_height = clamped.3;
        }

        let mut descendants = Vec::new();
        collect_descendant_ids(element_id, &children_map, &mut descendants);

        let delta_x = final_x - current.x;
        let delta_y = final_y - current.y;
        let geometry_changed = delta_x.abs() > 0.001
            || delta_y.abs() > 0.001
            || (final_width - current.width).abs() > 0.001
            || (final_height - current.height).abs() > 0.001
            || (rotation - current.rotation).abs() > 0.001;

        let mut updates: HashMap<Uuid, (f32, f32, f32, f32, f32)> = HashMap::new();
        updates.insert(
            element_id,
            (final_x, final_y, final_width, final_height, rotation),
        );

        if !descendants.is_empty() && geometry_changed {
            for descendant_id in &descendants {
                let Some(parent_id) = parent_map.get(descendant_id).and_then(|id| *id) else {
                    continue;
                };

                let Some((desc_x, desc_y, desc_w, desc_h, desc_r)) =
                    updates.get(descendant_id).copied().or_else(|| {
                        element_map.get(descendant_id).map(|element| {
                            (
                                element.x,
                                element.y,
                                element.width,
                                element.height,
                                element.rotation,
                            )
                        })
                    })
                else {
                    continue;
                };

                let Some((old_parent_x, old_parent_y, old_parent_w, old_parent_h, old_parent_r)) =
                    element_map.get(&parent_id).map(|parent| {
                        (
                            parent.x,
                            parent.y,
                            parent.width,
                            parent.height,
                            parent.rotation,
                        )
                    })
                else {
                    continue;
                };

                let Some((parent_x, parent_y, parent_w, parent_h, parent_r)) =
                    updates.get(&parent_id).copied().or_else(|| {
                        element_map.get(&parent_id).map(|parent| {
                            (
                                parent.x,
                                parent.y,
                                parent.width,
                                parent.height,
                                parent.rotation,
                            )
                        })
                    })
                else {
                    continue;
                };

                let transformed = transform_descendant_geometry_with_parent(
                    desc_x,
                    desc_y,
                    desc_w,
                    desc_h,
                    desc_r,
                    old_parent_x,
                    old_parent_y,
                    old_parent_w,
                    old_parent_h,
                    old_parent_r,
                    parent_x,
                    parent_y,
                    parent_w,
                    parent_h,
                    parent_r,
                );

                let clamped = clamp_to_parent_bounds(
                    transformed.0,
                    transformed.1,
                    transformed.2,
                    transformed.3,
                    transformed.4,
                    parent_x,
                    parent_y,
                    parent_w,
                    parent_h,
                    parent_r,
                );
                updates.insert(
                    *descendant_id,
                    (clamped.0, clamped.1, clamped.2, clamped.3, transformed.4),
                );
            }
        }

        let Some(root) = self.active_page_root_mut() else {
            return false;
        };

        let mut any_updated = false;
        let mut apply_order = Vec::with_capacity(1 + descendants.len());
        apply_order.push(element_id);
        apply_order.extend(descendants);

        for apply_id in apply_order {
            let Some((nx, ny, nw, nh, nr)) = updates.get(&apply_id).copied() else {
                continue;
            };

            if let Some(previous) = element_map.get(&apply_id) {
                if (previous.x - nx).abs() > 0.001
                    || (previous.y - ny).abs() > 0.001
                    || (previous.width - nw).abs() > 0.001
                    || (previous.height - nh).abs() > 0.001
                    || (previous.rotation - nr).abs() > 0.001
                {
                    any_updated = true;
                }
            }

            if let Some(element) = find_element_recursive_mut(&mut root.children, apply_id) {
                set_element_geometry(element, nx, ny, nw, nh, nr);
            }
        }

        any_updated
    }

    pub fn toggle_element_flip_on_active_page(
        &mut self,
        element_id: Uuid,
        horizontal: bool,
    ) -> bool {
        let Some(root) = self.active_page_root_mut() else {
            return false;
        };
        let Some(element) = find_element_recursive_mut(&mut root.children, element_id) else {
            return false;
        };
        let key = if horizontal {
            "flip_horizontal"
        } else {
            "flip_vertical"
        };
        let next = !element
            .properties
            .get(key)
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
        element
            .properties
            .insert(key.to_string(), serde_json::json!(next));
        true
    }

    pub fn update_element_text_on_active_page(&mut self, element_id: Uuid, text: &str) -> bool {
        let Some(root) = self.active_page_root_mut() else {
            return false;
        };

        let Some(element) = find_element_recursive_mut(&mut root.children, element_id) else {
            return false;
        };

        element
            .properties
            .insert("text".to_string(), serde_json::json!(text));
        true
    }

    pub fn update_element_style_on_active_page(
        &mut self,
        element_id: Uuid,
        background: &str,
        border_color: &str,
        border_width: f32,
        border_radius: f32,
        font_size: f32,
        text_color: &str,
        font_family: &str,
        text_wrap: &str,
        placeholder: &str,
        background_image: &str,
        image_src: &str,
        inherit_text_style: bool,
        checked: bool,
        checkbox_box_side: &str,
        checkbox_check_color: &str,
        checkbox_box_color: &str,
        checkbox_box_border_color: &str,
        checkbox_box_border_width: f32,
        checkbox_space_between: bool,
    ) -> bool {
        let Some(root) = self.active_page_root_mut() else {
            return false;
        };

        let Some(element) = find_element_recursive_mut(&mut root.children, element_id) else {
            return false;
        };

        let previous_background_image = get_string_property(element, "background_image");
        let previous_image_src = get_string_property(element, "image_src");

        let element_type = element
            .properties
            .get("element_type")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        let is_container = matches!(
            element_type,
            "div" | "flex-container" | "grid-container" | "stack-container"
        );
        let is_image_component = element_type == "image";
        let is_checkbox_component = element_type == "checkbox";
        let normalized_background = if matches!(element_type, "text" | "label") {
            "#0000"
        } else {
            background
        };
        let normalized_wrap = match text_wrap.trim().to_ascii_lowercase().as_str() {
            "nowrap" | "no-wrap" => "nowrap",
            _ => "wrap",
        };
        let normalized_font_family = if font_family.trim().is_empty() {
            "Sans"
        } else {
            font_family.trim()
        };
        let normalized_background_image = if is_container {
            normalized_image_asset_path(background_image, false)
        } else {
            String::new()
        };
        let normalized_image_src = if is_image_component {
            normalized_image_asset_path(image_src, true)
        } else {
            String::new()
        };
        let normalized_checkbox_box_side = if is_checkbox_component {
            match checkbox_box_side.trim().to_ascii_lowercase().as_str() {
                "right" | "end" => "right".to_string(),
                _ => "left".to_string(),
            }
        } else {
            "left".to_string()
        };
        let normalized_checkbox_check_color =
            if is_checkbox_component && !checkbox_check_color.trim().is_empty() {
                checkbox_check_color.trim().to_string()
            } else {
                "#f5f5f5".to_string()
            };
        let normalized_checkbox_box_color =
            if is_checkbox_component && !checkbox_box_color.trim().is_empty() {
                checkbox_box_color.trim().to_string()
            } else {
                "#151515".to_string()
            };
        let normalized_checkbox_box_border_color =
            if is_checkbox_component && !checkbox_box_border_color.trim().is_empty() {
                checkbox_box_border_color.trim().to_string()
            } else {
                "#4a4a4a".to_string()
            };
        let normalized_checkbox_box_border_width = if is_checkbox_component {
            checkbox_box_border_width.max(0.0)
        } else {
            1.0
        };

        element.properties.insert(
            "background".to_string(),
            serde_json::json!(normalized_background),
        );
        element
            .properties
            .insert("border_color".to_string(), serde_json::json!(border_color));
        element.properties.insert(
            "border_width".to_string(),
            serde_json::json!(border_width.max(0.0)),
        );
        element.properties.insert(
            "border_radius".to_string(),
            serde_json::json!(border_radius.max(0.0)),
        );
        element.properties.insert(
            "font_size".to_string(),
            serde_json::json!(font_size.max(1.0)),
        );
        element
            .properties
            .insert("text_color".to_string(), serde_json::json!(text_color));
        element.properties.insert(
            "font_family".to_string(),
            serde_json::json!(normalized_font_family),
        );
        element
            .properties
            .insert("text_wrap".to_string(), serde_json::json!(normalized_wrap));
        element
            .properties
            .insert("placeholder".to_string(), serde_json::json!(placeholder));
        element.properties.insert(
            "background_image".to_string(),
            serde_json::json!(normalized_background_image),
        );
        element.properties.insert(
            "image_src".to_string(),
            serde_json::json!(normalized_image_src),
        );
        element.properties.insert(
            "inherit_text_style".to_string(),
            serde_json::json!(inherit_text_style),
        );
        element
            .properties
            .insert("checked".to_string(), serde_json::json!(checked));
        element.properties.insert(
            "checkbox_box_side".to_string(),
            serde_json::json!(normalized_checkbox_box_side),
        );
        element.properties.insert(
            "checkbox_check_color".to_string(),
            serde_json::json!(normalized_checkbox_check_color),
        );
        element.properties.insert(
            "checkbox_box_color".to_string(),
            serde_json::json!(normalized_checkbox_box_color),
        );
        element.properties.insert(
            "checkbox_box_border_color".to_string(),
            serde_json::json!(normalized_checkbox_box_border_color),
        );
        element.properties.insert(
            "checkbox_box_border_width".to_string(),
            serde_json::json!(normalized_checkbox_box_border_width),
        );
        element.properties.insert(
            "checkbox_space_between".to_string(),
            serde_json::json!(is_checkbox_component && checkbox_space_between),
        );

        let mut candidates = Vec::new();
        if let Some(previous) = previous_background_image {
            if previous != normalized_background_image {
                candidates.push(previous);
            }
        }
        if let Some(previous) = previous_image_src {
            if previous != normalized_image_src {
                candidates.push(previous);
            }
        }
        self.prune_unused_image_assets(&candidates);

        true
    }

    pub fn relayout_container_on_active_page(&mut self, container_id: Uuid) -> bool {
        self.layout_children_in_parent_on_active_page(container_id)
    }

    pub fn managed_layout_parent_on_active_page(&self, element_id: Uuid) -> Option<Uuid> {
        let parent_id = self.element_parent_on_active_page(element_id)?;
        let parent = self.get_element_on_active_page(parent_id)?;
        let mode = effective_container_mode_for_canvas(&parent);
        (mode.is_managed() && !container_allows_absolute_children(&parent)).then_some(parent_id)
    }

    pub fn container_uses_managed_layout_on_active_page(&self, element_id: Uuid) -> bool {
        self.get_element_on_active_page(element_id)
            .map(|element| effective_container_mode_for_canvas(&element).is_managed())
            .unwrap_or(false)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_element_container_settings_on_active_page(
        &mut self,
        element_id: Uuid,
        container_mode: &str,
        allow_absolute_children: bool,
        layout_padding: f32,
        layout_padding_left: f32,
        layout_padding_right: f32,
        layout_padding_top: f32,
        layout_padding_bottom: f32,
        layout_spacing: f32,
        layout_margin: f32,
        layout_margin_left: f32,
        layout_margin_right: f32,
        layout_margin_top: f32,
        layout_margin_bottom: f32,
        layout_order: f32,
        stack_alignment: &str,
        flex_direction: &str,
        flex_wrap: &str,
        justify_items: &str,
        justify_content: &str,
        align_items: &str,
        align_content: &str,
        place_items: &str,
        flex_flow: &str,
        grid_template_columns: &str,
        grid_template_rows: &str,
        grid_template_areas: &str,
    ) -> bool {
        if !layout_padding.is_finite()
            || !layout_padding_left.is_finite()
            || !layout_padding_right.is_finite()
            || !layout_padding_top.is_finite()
            || !layout_padding_bottom.is_finite()
            || !layout_spacing.is_finite()
            || !layout_margin.is_finite()
            || !layout_margin_left.is_finite()
            || !layout_margin_right.is_finite()
            || !layout_margin_top.is_finite()
            || !layout_margin_bottom.is_finite()
            || !layout_order.is_finite()
        {
            return false;
        }

        let Some(current) = self.get_element_on_active_page(element_id) else {
            return false;
        };
        let element_type = current.element_type.clone();
        let is_container = Self::is_layout_container_type(element_type.as_str());

        let normalized_container_mode = if element_type == "div" {
            normalize_container_mode(container_mode)
        } else {
            effective_container_mode_for_canvas(&current)
        };
        let normalized_layout_padding = layout_padding.clamp(0.0, 2_000.0);
        let normalized_layout_padding_left = layout_padding_left.clamp(0.0, 2_000.0);
        let normalized_layout_padding_right = layout_padding_right.clamp(0.0, 2_000.0);
        let normalized_layout_padding_top = layout_padding_top.clamp(0.0, 2_000.0);
        let normalized_layout_padding_bottom = layout_padding_bottom.clamp(0.0, 2_000.0);
        let normalized_layout_spacing = layout_spacing.clamp(0.0, 2_000.0);
        let normalized_layout_margin = layout_margin.clamp(0.0, 2_000.0);
        let normalized_layout_margin_left = layout_margin_left.clamp(0.0, 2_000.0);
        let normalized_layout_margin_right = layout_margin_right.clamp(0.0, 2_000.0);
        let normalized_layout_margin_top = layout_margin_top.clamp(0.0, 2_000.0);
        let normalized_layout_margin_bottom = layout_margin_bottom.clamp(0.0, 2_000.0);
        let normalized_layout_order = layout_order.clamp(-9_999.0, 9_999.0);
        let normalized_stack_alignment = normalize_stack_alignment(stack_alignment);
        let mut normalized_flex_direction = normalize_flex_direction(flex_direction);
        let mut normalized_flex_wrap = normalize_flex_wrap(flex_wrap);
        let (flow_direction, flow_wrap) = parse_flex_flow(
            flex_flow,
            normalized_flex_direction.as_str(),
            normalized_flex_wrap.as_str(),
        );
        normalized_flex_direction = flow_direction;
        normalized_flex_wrap = flow_wrap;
        let normalized_flex_flow = format!("{normalized_flex_direction} {normalized_flex_wrap}");
        let normalized_justify_items = normalize_item_alignment(justify_items);
        let normalized_justify_content = normalize_justify_content(justify_content);
        let normalized_align_items = normalize_item_alignment(align_items);
        let normalized_align_content = normalize_align_content(align_content);
        let (place_justify_items, place_align_items) = parse_place_items(
            place_items,
            normalized_justify_items.as_str(),
            normalized_align_items.as_str(),
        );
        let normalized_place_items = format!("{place_justify_items} {place_align_items}");
        let normalized_grid_template_columns = if grid_template_columns.trim().is_empty() {
            "1fr 1fr".to_string()
        } else {
            grid_template_columns.trim().to_string()
        };
        let normalized_grid_template_rows = if grid_template_rows.trim().is_empty() {
            "auto auto".to_string()
        } else {
            grid_template_rows.trim().to_string()
        };
        let normalized_grid_template_areas = grid_template_areas.trim().to_string();
        let normalized_allow_absolute_children =
            normalized_container_mode == ContainerMode::Stack && allow_absolute_children;

        {
            let Some(root) = self.active_page_root_mut() else {
                return false;
            };
            let Some(element) = find_element_recursive_mut(&mut root.children, element_id) else {
                return false;
            };

            element.properties.insert(
                "layout_padding".to_string(),
                serde_json::json!(normalized_layout_padding),
            );
            element.properties.insert(
                "layout_padding_left".to_string(),
                serde_json::json!(normalized_layout_padding_left),
            );
            element.properties.insert(
                "layout_padding_right".to_string(),
                serde_json::json!(normalized_layout_padding_right),
            );
            element.properties.insert(
                "layout_padding_top".to_string(),
                serde_json::json!(normalized_layout_padding_top),
            );
            element.properties.insert(
                "layout_padding_bottom".to_string(),
                serde_json::json!(normalized_layout_padding_bottom),
            );
            element.properties.insert(
                "layout_spacing".to_string(),
                serde_json::json!(normalized_layout_spacing),
            );
            element.properties.insert(
                "layout_margin".to_string(),
                serde_json::json!(normalized_layout_margin),
            );
            element.properties.insert(
                "layout_margin_left".to_string(),
                serde_json::json!(normalized_layout_margin_left),
            );
            element.properties.insert(
                "layout_margin_right".to_string(),
                serde_json::json!(normalized_layout_margin_right),
            );
            element.properties.insert(
                "layout_margin_top".to_string(),
                serde_json::json!(normalized_layout_margin_top),
            );
            element.properties.insert(
                "layout_margin_bottom".to_string(),
                serde_json::json!(normalized_layout_margin_bottom),
            );
            element.properties.insert(
                "layout_order".to_string(),
                serde_json::json!(normalized_layout_order),
            );

            if is_container {
                if element_type == "div" {
                    element.properties.insert(
                        "container_mode".to_string(),
                        serde_json::json!(normalized_container_mode.as_str()),
                    );
                }

                match normalized_container_mode {
                    ContainerMode::Absolute => {
                        element.properties.remove("display");
                        element
                            .properties
                            .insert("responsive_mode".to_string(), serde_json::json!("manual"));
                        element.properties.insert(
                            "positioning_mode".to_string(),
                            serde_json::json!("absolute-children"),
                        );
                    }
                    ContainerMode::Stack => {
                        element
                            .properties
                            .insert("display".to_string(), serde_json::json!("block"));
                        element.properties.insert(
                            "responsive_mode".to_string(),
                            serde_json::json!("layout-managed"),
                        );
                        element.properties.insert(
                            "positioning_mode".to_string(),
                            serde_json::json!(if normalized_allow_absolute_children {
                                "absolute-children"
                            } else {
                                "stack-only"
                            }),
                        );
                    }
                    ContainerMode::Flex => {
                        element
                            .properties
                            .insert("display".to_string(), serde_json::json!("flex"));
                        element.properties.insert(
                            "responsive_mode".to_string(),
                            serde_json::json!("layout-managed"),
                        );
                        element.properties.insert(
                            "positioning_mode".to_string(),
                            serde_json::json!("flex-only"),
                        );
                    }
                    ContainerMode::Grid => {
                        element
                            .properties
                            .insert("display".to_string(), serde_json::json!("grid"));
                        element.properties.insert(
                            "responsive_mode".to_string(),
                            serde_json::json!("layout-managed"),
                        );
                        element.properties.insert(
                            "positioning_mode".to_string(),
                            serde_json::json!("grid-only"),
                        );
                    }
                }

                element.properties.insert(
                    "allow_absolute_children".to_string(),
                    serde_json::json!(normalized_allow_absolute_children),
                );
                element.properties.insert(
                    "stack_alignment".to_string(),
                    serde_json::json!(normalized_stack_alignment),
                );
                element.properties.insert(
                    "flex_direction".to_string(),
                    serde_json::json!(normalized_flex_direction),
                );
                element.properties.insert(
                    "flex_wrap".to_string(),
                    serde_json::json!(normalized_flex_wrap),
                );
                element.properties.insert(
                    "justify_items".to_string(),
                    serde_json::json!(place_justify_items),
                );
                element.properties.insert(
                    "justify_content".to_string(),
                    serde_json::json!(normalized_justify_content),
                );
                element.properties.insert(
                    "align_items".to_string(),
                    serde_json::json!(place_align_items),
                );
                element.properties.insert(
                    "align_content".to_string(),
                    serde_json::json!(normalized_align_content),
                );
                element.properties.insert(
                    "place_items".to_string(),
                    serde_json::json!(normalized_place_items),
                );
                element.properties.insert(
                    "flex_flow".to_string(),
                    serde_json::json!(normalized_flex_flow),
                );
                element.properties.insert(
                    "grid_template_columns".to_string(),
                    serde_json::json!(normalized_grid_template_columns.clone()),
                );
                element.properties.insert(
                    "grid_template_rows".to_string(),
                    serde_json::json!(normalized_grid_template_rows.clone()),
                );
                element.properties.insert(
                    "grid_template_areas".to_string(),
                    serde_json::json!(normalized_grid_template_areas),
                );
                element.properties.insert(
                    "grid_columns".to_string(),
                    serde_json::json!(normalized_grid_template_columns),
                );
                element.properties.insert(
                    "grid_rows".to_string(),
                    serde_json::json!(normalized_grid_template_rows),
                );
            }
        }

        let mut changed = true;
        if is_container && normalized_container_mode.is_managed() {
            changed |= self.layout_children_in_parent_on_active_page(element_id);
        }
        if let Some(parent_id) = self.managed_layout_parent_on_active_page(element_id) {
            changed |= self.layout_children_in_parent_on_active_page(parent_id);
        }
        changed
    }

    pub fn set_element_image_source_on_active_page(
        &mut self,
        element_id: Uuid,
        image_src: &str,
    ) -> bool {
        let Some(root) = self.active_page_root_mut() else {
            return false;
        };

        let Some(element) = find_element_recursive_mut(&mut root.children, element_id) else {
            return false;
        };

        let element_type = element
            .properties
            .get("element_type")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        if element_type != "image" {
            return false;
        }

        let previous_path = element
            .properties
            .get("image_src")
            .and_then(|value| value.as_str())
            .map(|value| value.to_string());
        let normalized = normalized_image_asset_path(image_src, true);
        element.properties.insert(
            "image_src".to_string(),
            serde_json::json!(normalized.clone()),
        );
        if let Some(previous) = previous_path {
            if previous != normalized {
                self.prune_unused_image_assets(&[previous]);
            }
        }
        true
    }

    pub fn update_element_name_on_active_page(
        &mut self,
        element_id: Uuid,
        requested_name: &str,
    ) -> bool {
        let fallback_type = self
            .get_element_on_active_page(element_id)
            .map(|element| element.element_type)
            .unwrap_or_else(|| "component".to_string());
        let unique_name = self.unique_element_name_on_active_page(
            requested_name,
            &fallback_type,
            Some(element_id),
        );

        let Some(root) = self.active_page_root_mut() else {
            return false;
        };
        let Some(element) = find_element_recursive_mut(&mut root.children, element_id) else {
            return false;
        };
        element
            .properties
            .insert("name".to_string(), serde_json::json!(unique_name));
        true
    }

    pub fn update_element_parent_on_active_page(
        &mut self,
        element_id: Uuid,
        parent_id: Option<Uuid>,
    ) -> bool {
        let previous_parent = self.element_parent_on_active_page(element_id);
        if !self.update_element_parent_property(element_id, parent_id) {
            return false;
        }

        if let Some(element) = self.get_element_on_active_page(element_id) {
            let _ = self.update_element_geometry_on_active_page(
                element_id,
                element.x,
                element.y,
                element.width,
                element.height,
                element.rotation,
            );
        }

        if let Some(previous_parent) = previous_parent {
            if Some(previous_parent) != parent_id {
                let _ = self.layout_children_in_parent_on_active_page(previous_parent);
            }
        }
        if let Some(parent_id) = parent_id {
            let _ = self.layout_children_in_parent_on_active_page(parent_id);
        }

        true
    }

    pub fn group_elements_on_active_page(&mut self, parent_id: Uuid, child_ids: &[Uuid]) -> bool {
        let parent_exists = self
            .active_page_root()
            .and_then(|root| find_element_recursive(&root.children, parent_id))
            .is_some();
        if !parent_exists {
            return false;
        }

        let mut changed = false;
        for child_id in child_ids
            .iter()
            .copied()
            .filter(|child_id| *child_id != parent_id)
        {
            if self.update_element_parent_property(child_id, Some(parent_id)) {
                if let Some(child) = self.get_element_on_active_page(child_id) {
                    let _ = self.update_element_geometry_on_active_page(
                        child_id,
                        child.x,
                        child.y,
                        child.width,
                        child.height,
                        child.rotation,
                    );
                }
                changed = true;
            }
        }
        if changed {
            let _ = self.layout_children_in_parent_on_active_page(parent_id);
        }
        changed
    }

    pub fn ungroup_element_on_active_page(&mut self, element_id: Uuid) -> bool {
        let parent_id = self.element_parent_on_active_page(element_id);
        let next_parent = parent_id.and_then(|parent| self.element_parent_on_active_page(parent));

        if !self.update_element_parent_property(element_id, next_parent) {
            return false;
        }

        if let Some(element) = self.get_element_on_active_page(element_id) {
            let _ = self.update_element_geometry_on_active_page(
                element_id,
                element.x,
                element.y,
                element.width,
                element.height,
                element.rotation,
            );
        }

        if let Some(parent_id) = parent_id {
            let _ = self.layout_children_in_parent_on_active_page(parent_id);
        }
        if let Some(next_parent) = next_parent {
            let _ = self.layout_children_in_parent_on_active_page(next_parent);
        }

        true
    }

    pub fn element_parent_on_active_page(&self, element_id: Uuid) -> Option<Uuid> {
        let root = self.active_page_root()?;
        let element = find_element_recursive(&root.children, element_id)?;
        let parent = parse_parent_property(element)?;
        find_element_recursive(&root.children, parent).map(|_| parent)
    }

    pub fn get_element_on_active_page(&self, element_id: Uuid) -> Option<CanvasElementData> {
        let root = self.active_page_root()?;
        let element = find_element_recursive(&root.children, element_id)?;
        Some(CanvasElementData::from_ui_element(element))
    }

    pub fn remove_element_on_active_page(&mut self, element_id: Uuid) -> bool {
        let all_elements = self.active_page_elements();
        let element_map: HashMap<Uuid, CanvasElementData> = all_elements
            .iter()
            .cloned()
            .map(|element| (element.id, element))
            .collect();
        if !element_map.contains_key(&element_id) {
            return false;
        }
        let parent_id = self.element_parent_on_active_page(element_id);

        let mut parent_map: HashMap<Uuid, Option<Uuid>> = HashMap::new();
        let mut children_map: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
        for element in &all_elements {
            let parent_id = parse_parent_property_from_canvas(element).filter(|parent_id| {
                *parent_id != element.id && element_map.contains_key(parent_id)
            });
            parent_map.insert(element.id, parent_id);
            if let Some(parent_id) = parent_id {
                children_map.entry(parent_id).or_default().push(element.id);
            }
        }

        let mut remove_ids = vec![element_id];
        collect_descendant_ids(element_id, &children_map, &mut remove_ids);
        let remove_set: HashSet<Uuid> = remove_ids.into_iter().collect();
        let mut removed_image_paths = Vec::new();
        for remove_id in &remove_set {
            if let Some(element) = element_map.get(remove_id) {
                let image_src = canvas_prop_str(element, "image_src", "");
                if !image_src.trim().is_empty() {
                    removed_image_paths.push(image_src);
                }
                let background_image = canvas_prop_str(element, "background_image", "");
                if !background_image.trim().is_empty() {
                    removed_image_paths.push(background_image);
                }
            }
        }

        let Some(root) = self.active_page_root_mut() else {
            return false;
        };
        let removed = retain_elements_recursive(&mut root.children, &remove_set);
        if removed {
            if let Some(parent_id) = parent_id {
                let _ = self.layout_children_in_parent_on_active_page(parent_id);
            }
            self.prune_unused_image_assets(&removed_image_paths);
        }
        removed
    }

    pub fn snapshot_binary(&self) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let snapshot = ProjectSnapshotState {
            project_file: self.project_file.clone(),
            active_page_index: self.active_page_index,
        };
        shared::to_msgpack(&snapshot).map_err(|err| {
            Box::new(std::io::Error::other(err.to_string())) as Box<dyn std::error::Error>
        })
    }

    pub fn restore_from_binary(&mut self, bytes: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
        let snapshot: ProjectSnapshotState = shared::from_msgpack(bytes).map_err(|err| {
            Box::new(std::io::Error::other(err.to_string())) as Box<dyn std::error::Error>
        })?;

        let active_page_index = if snapshot.project_file.ui_data.pages.is_empty() {
            0
        } else {
            snapshot
                .active_page_index
                .min(snapshot.project_file.ui_data.pages.len() - 1)
        };

        self.project_file = snapshot.project_file;
        self.active_page_index = active_page_index;
        self.sync_document_model();
        Ok(())
    }

    fn migrate_comment_images_to_assets(&mut self) {
        let images_dir = project_images_dir_from_assets_root(&self.assets_root);
        let mut ensured_dir = false;

        for page in &mut self.project_file.ui_data.pages {
            for comment in &mut page.comments {
                let Some(image) = comment.image.as_mut() else {
                    continue;
                };

                if !image.path.trim().is_empty() {
                    continue;
                }
                if image.rgba.is_empty() || image.width == 0 || image.height == 0 {
                    continue;
                }

                let hash = hash_bytes_fnv1a(&image.rgba);
                let file_name = format!("comment-{hash:016x}.png");
                let destination = images_dir.join(&file_name);

                if !destination.exists() {
                    let Some(rgba_image) =
                        RgbaImage::from_raw(image.width, image.height, image.rgba.clone())
                    else {
                        continue;
                    };

                    if !ensured_dir {
                        if let Err(err) = fs::create_dir_all(&images_dir) {
                            eprintln!(
                                "Failed to create project image assets directory {}: {err}",
                                images_dir.display()
                            );
                            return;
                        }
                        ensured_dir = true;
                    }

                    if let Err(err) = rgba_image.save_with_format(&destination, ImageFormat::Png) {
                        eprintln!(
                            "Failed to migrate embedded comment image to {}: {err}",
                            destination.display()
                        );
                        continue;
                    }
                }

                image.path = project_image_asset_path(&file_name);
                image.rgba.clear();
            }
        }
    }

    /// Get the .spx file path for this project.
    pub fn spx_file_path(&self) -> PathBuf {
        self.file_path.clone()
    }

    pub fn assets_root_dir(&self) -> &Path {
        &self.assets_root
    }

    /// Save project to .spx archive file.
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = Path::new(&self.file_path);
        operations::save_project_with_assets(&self.project_file, path, &self.assets_root).map_err(
            |e| Box::new(std::io::Error::other(e.to_string())) as Box<dyn std::error::Error>,
        )
    }

    /// Load project from .spx archive file.
    pub fn load(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let file_path = PathBuf::from(path);
        let path_ref = Path::new(&file_path);

        let project_file = operations::load_project(path_ref).map_err(|e| {
            Box::new(std::io::Error::other(e.to_string())) as Box<dyn std::error::Error>
        })?;

        let active_page_index = 0;

        let assets_root = std::env::temp_dir().join(format!("snappix-assets-{}", Uuid::new_v4()));
        if let Err(err) = operations::extract_project_assets(path_ref, &assets_root) {
            eprintln!("Failed to extract project assets: {err}");
        }

        let mut project = Self {
            project_file,
            file_path,
            active_page_index,
            assets_root,
        };
        project.migrate_comment_images_to_assets();
        project.sync_document_model();
        Ok(project)
    }
}

fn sanitize_blueprint_symbol(name: &str) -> String {
    let mut result = String::new();
    for ch in name.trim().chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            result.push(ch);
        } else if ch.is_whitespace() || ch == '-' {
            result.push('_');
        }
    }
    result.trim_matches('_').to_string()
}

fn parse_blueprint_pin_type_name(type_name: &str) -> Option<BlueprintPinType> {
    match type_name.trim().to_ascii_lowercase().as_str() {
        "bool" => Some(BlueprintPinType::Bool),
        "i64" | "int" | "integer" => Some(BlueprintPinType::Int),
        "f64" | "float" | "number" => Some(BlueprintPinType::Float),
        "string" | "str" => Some(BlueprintPinType::String),
        "color" => Some(BlueprintPinType::Color),
        "array" => Some(BlueprintPinType::Array),
        "vector" | "vec" => Some(BlueprintPinType::Vector),
        "set" | "hashset" => Some(BlueprintPinType::HashSet),
        "hashmap" | "map" => Some(BlueprintPinType::HashMap),
        "element" | "uielement" => Some(BlueprintPinType::UiElementRef),
        "page" => Some(BlueprintPinType::PageRef),
        "api" => Some(BlueprintPinType::ApiRef),
        _ => None,
    }
}

fn blueprint_node_size(kind: &BlueprintNodeKind) -> (i32, i32) {
    match kind {
        BlueprintNodeKind::VariableGet { .. } => (190, 48),
        BlueprintNodeKind::VariableSet { .. } => (210, 96),
        BlueprintNodeKind::Functional { node_id } if node_id == "if_statement" => (230, 128),
        BlueprintNodeKind::UiEvent { .. } => (190, 112),
        _ => (220, 112),
    }
}

fn nearest_free_blueprint_position(
    nodes: &[BlueprintNode],
    preferred: BlueprintPoint,
    size: (i32, i32),
) -> BlueprintPoint {
    let step_x = 260;
    let step_y = 150;
    let margin = 24;

    for radius in 0i32..12 {
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if radius > 0 && dx.abs() != radius && dy.abs() != radius {
                    continue;
                }
                let candidate = BlueprintPoint {
                    x: preferred.x + dx * step_x,
                    y: preferred.y + dy * step_y,
                };
                if !blueprint_position_overlaps(nodes, candidate, size, margin) {
                    return candidate;
                }
            }
        }
    }

    BlueprintPoint {
        x: preferred.x + step_x,
        y: preferred.y,
    }
}

fn blueprint_position_overlaps(
    nodes: &[BlueprintNode],
    candidate: BlueprintPoint,
    size: (i32, i32),
    margin: i32,
) -> bool {
    let left = candidate.x - margin;
    let top = candidate.y - margin;
    let right = candidate.x + size.0 + margin;
    let bottom = candidate.y + size.1 + margin;

    nodes.iter().any(|node| {
        let node_size = blueprint_node_size(&node.kind);
        let node_left = node.position.x;
        let node_top = node.position.y;
        let node_right = node.position.x + node_size.0;
        let node_bottom = node.position.y + node_size.1;
        right > node_left && left < node_right && bottom > node_top && top < node_bottom
    })
}

/// Project manager singleton.
pub struct ProjectManager {
    current_project: Option<Project>,
}

impl ProjectManager {
    pub fn new() -> Self {
        Self {
            current_project: None,
        }
    }

    /// Create a new project.
    pub fn create_project(
        &mut self,
        name: &str,
        path: &str,
        platform: Platform,
        dev_mode: DevMode,
        initial_page_size: PageSize,
        custom_width: u32,
        custom_height: u32,
    ) -> &Project {
        let project = Project::new(
            name,
            path,
            platform,
            dev_mode,
            initial_page_size,
            custom_width,
            custom_height,
        );
        self.current_project = Some(project);
        self.current_project.as_ref().expect("project just set")
    }

    /// Load an existing project.
    pub fn load_project(&mut self, path: &str) -> Result<&Project, Box<dyn std::error::Error>> {
        let project = Project::load(path)?;
        self.current_project = Some(project);
        Ok(self.current_project.as_ref().expect("project just set"))
    }

    pub fn current_project_mut(&mut self) -> Option<&mut Project> {
        self.current_project.as_mut()
    }
}

impl Default for ProjectManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_blueprint::{BlueprintLink, BlueprintNode};
    use tempfile::tempdir;

    fn assert_close(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() <= 0.01,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn descendant_transform_scales_in_parent_local_space() {
        let old_parent_x = 100.0;
        let old_parent_y = 100.0;
        let old_parent_w = 200.0;
        let old_parent_h = 100.0;
        let old_parent_rotation = 45.0;
        let old_parent_center_x = old_parent_x + old_parent_w / 2.0;
        let old_parent_center_y = old_parent_y + old_parent_h / 2.0;
        let local_offset = (30.0, -10.0);
        let (world_offset_x, world_offset_y) =
            rotate_vector(local_offset.0, local_offset.1, old_parent_rotation);
        let child_width = 20.0;
        let child_height = 10.0;
        let child_center_x = old_parent_center_x + world_offset_x;
        let child_center_y = old_parent_center_y + world_offset_y;

        let result = transform_descendant_geometry_with_parent(
            child_center_x - child_width / 2.0,
            child_center_y - child_height / 2.0,
            child_width,
            child_height,
            45.0,
            old_parent_x,
            old_parent_y,
            old_parent_w,
            old_parent_h,
            old_parent_rotation,
            100.0,
            100.0,
            400.0,
            200.0,
            45.0,
        );

        let expected_center_x = 300.0 + rotate_vector(60.0, -20.0, 45.0).0;
        let expected_center_y = 200.0 + rotate_vector(60.0, -20.0, 45.0).1;
        assert_close(result.0 + result.2 / 2.0, expected_center_x);
        assert_close(result.1 + result.3 / 2.0, expected_center_y);
        assert_close(result.2, 40.0);
        assert_close(result.3, 20.0);
        assert_close(result.4, 45.0);
    }

    #[test]
    fn descendant_transform_rotates_with_parent() {
        let result = transform_descendant_geometry_with_parent(
            240.0, 145.0, 20.0, 10.0, 0.0, 100.0, 100.0, 200.0, 100.0, 0.0, 100.0, 100.0, 200.0,
            100.0, 90.0,
        );

        assert_close(result.0 + result.2 / 2.0, 200.0);
        assert_close(result.1 + result.3 / 2.0, 200.0);
        assert_close(result.2, 20.0);
        assert_close(result.3, 10.0);
        assert_close(result.4, 90.0);
    }

    #[test]
    fn clamp_to_parent_bounds_preserves_valid_child_in_rotated_parent() {
        let parent_x = 100.0;
        let parent_y = 100.0;
        let parent_w = 200.0;
        let parent_h = 100.0;
        let parent_rotation = 45.0;
        let parent_center_x = parent_x + parent_w / 2.0;
        let parent_center_y = parent_y + parent_h / 2.0;
        let child_width = 20.0;
        let child_height = 10.0;
        let (world_offset_x, world_offset_y) = rotate_vector(-80.0, 0.0, parent_rotation);
        let child_center_x = parent_center_x + world_offset_x;
        let child_center_y = parent_center_y + world_offset_y;
        let child_x = child_center_x - child_width / 2.0;
        let child_y = child_center_y - child_height / 2.0;

        let result = clamp_to_parent_bounds(
            child_x,
            child_y,
            child_width,
            child_height,
            45.0,
            parent_x,
            parent_y,
            parent_w,
            parent_h,
            parent_rotation,
        );

        assert_close(result.0, child_x);
        assert_close(result.1, child_y);
        assert_close(result.2, child_width);
        assert_close(result.3, child_height);
    }

    #[test]
    fn project_round_trips_blueprint_documents_and_workspace_tabs() {
        let temp = tempdir().expect("tempdir");
        let project_dir = temp.path().to_string_lossy().to_string();
        let mut project = Project::new(
            "SnappixBlueprint",
            &project_dir,
            Platform::Desktop,
            DevMode::Nodes,
            PageSize::Desktop,
            0,
            0,
        );

        project.set_active_page(0);
        let page_blueprint = project
            .page_blueprint_document_ref(0)
            .expect("page blueprint");
        let server_blueprint = project
            .server_blueprint_document_ref()
            .expect("server blueprint");

        assert!(project.open_document(page_blueprint.clone()));
        assert!(project.open_document(server_blueprint.clone()));
        project.save().expect("save project");

        let project_path = project.spx_file_path().to_string_lossy().to_string();
        let loaded = Project::load(&project_path).expect("load project");
        let labels: Vec<String> = loaded
            .open_documents()
            .iter()
            .filter_map(|document| loaded.document_label(document))
            .collect();

        assert_eq!(loaded.project_file.logic_data.documents.len(), 2);
        assert_eq!(
            labels,
            vec![
                "Main".to_string(),
                "main.blp".to_string(),
                "server.blp".to_string()
            ]
        );
        assert_eq!(loaded.active_document(), Some(&server_blueprint));
    }

    #[test]
    fn project_compiles_simple_page_blueprint_to_rust_workspace() {
        let temp = tempdir().expect("tempdir");
        let project_dir = temp.path().to_string_lossy().to_string();
        let mut project = Project::new(
            "SnappixCompile",
            &project_dir,
            Platform::Desktop,
            DevMode::Nodes,
            PageSize::Desktop,
            0,
            0,
        );

        let button = CanvasElementData::from_component_template("button", 24.0, 24.0);
        let button_id = button.id;
        assert!(project
            .add_element_to_active_page_with_parent(button, None)
            .is_some());

        let label = CanvasElementData::from_component_template("label", 24.0, 96.0);
        let label_id = label.id;
        assert!(project
            .add_element_to_active_page_with_parent(label, None)
            .is_some());

        let page_blueprint_id = match project
            .page_blueprint_document_ref(project.active_page_index())
            .expect("page blueprint")
        {
            EditorDocumentRef::PageBlueprint { document_id } => document_id,
            _ => unreachable!("expected page blueprint ref"),
        };

        let document = project
            .project_file
            .logic_data
            .documents
            .iter_mut()
            .find(|document| document.id == page_blueprint_id)
            .expect("blueprint document");
        document.graphs[0].entrypoints.clear();
        document.graphs[0].nodes.clear();
        document.graphs[0].links.clear();

        let event = BlueprintNode::ui_event(button_id, "clicked");
        let set_text = BlueprintNode::set_element_text(label_id);
        let literal = BlueprintNode::literal_string("Hello Snappix");

        document.graphs[0].entrypoints.push(event.id);
        document.graphs[0].nodes = vec![event.clone(), set_text.clone(), literal.clone()];
        document.graphs[0].links = vec![
            BlueprintLink::new(
                event.id,
                event.pin_named("then").expect("event exec").id,
                set_text.id,
                set_text.pin_named("in").expect("set text exec").id,
            ),
            BlueprintLink::new(
                literal.id,
                literal.pin_named("value").expect("literal value").id,
                set_text.id,
                set_text.pin_named("text").expect("set text input").id,
            ),
        ];

        let result = project.compile_blueprints();
        assert!(
            result.success,
            "expected blueprint compile success, diagnostics: {:?}",
            result.diagnostics
        );
        assert!(result
            .generated_files
            .iter()
            .any(|file| file.path.ends_with("runtime.rs")));
        assert!(result
            .source_map
            .spans
            .iter()
            .any(|span| span.node_id == set_text.id));
    }

    #[test]
    fn project_creates_variable_getter_and_setter_nodes_from_sidebar_requests() {
        let temp = tempdir().expect("tempdir");
        let project_dir = temp.path().to_string_lossy().to_string();
        let mut project = Project::new(
            "SnappixVariables",
            &project_dir,
            Platform::Desktop,
            DevMode::Nodes,
            PageSize::Desktop,
            0,
            0,
        );

        let blueprint_ref = project
            .page_blueprint_document_ref(project.active_page_index())
            .expect("page blueprint");
        assert!(project.open_document(blueprint_ref));

        let variable_id = project
            .add_local_variable_to_active_blueprint()
            .expect("variable");
        let getter_id = project
            .add_variable_node_to_active_blueprint(variable_id, "get", 0.0, 0.0)
            .expect("getter");
        let setter_id = project
            .add_variable_node_to_active_blueprint(variable_id, "set", 0.0, 0.0)
            .expect("setter");

        let nodes = project.active_blueprint_nodes();
        let getter = nodes.iter().find(|node| node.id == getter_id).unwrap();
        let setter = nodes.iter().find(|node| node.id == setter_id).unwrap();
        assert!(matches!(getter.kind, BlueprintNodeKind::VariableGet { .. }));
        assert!(matches!(setter.kind, BlueprintNodeKind::VariableSet { .. }));
        assert_ne!(getter.position, setter.position);

        assert!(project.rename_local_variable_in_active_blueprint(variable_id, "is_ready"));
        assert!(project.set_local_variable_type_in_active_blueprint(variable_id, "String"));
        let nodes = project.active_blueprint_nodes();
        let getter = nodes.iter().find(|node| node.id == getter_id).unwrap();
        assert_eq!(getter.title, "is_ready");
        assert_eq!(
            getter.pin_named("value").expect("getter value").data_type,
            BlueprintPinType::String
        );
    }
}
