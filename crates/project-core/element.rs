use std::collections::HashMap;

use core_ui_graphs::{
    layout::{
        AlignContent, AlignItems, FlexDirection, FlexLayout, FlexWrap, GridLayout, JustifyContent,
        LayoutStyles, SizeValue,
    },
    project::Page,
    ElementKind, UiElement,
};
use uuid::Uuid;

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

    pub fn display_name(element_type: &str) -> &'static str {
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

    pub fn name_seed(element_type: &str) -> String {
        match element_type {
            "flex-container" => "flex",
            "grid-container" => "grid",
            "stack-container" => "stack",
            other => other,
        }
        .replace('-', "")
        .to_lowercase()
    }

    pub fn template_size(element_type: &str) -> (f32, f32) {
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

    pub fn template_properties(element_type: &str) -> HashMap<String, serde_json::Value> {
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
        props.insert("opacity".to_string(), serde_json::json!(1.0));
        props.insert("display_mode".to_string(), serde_json::json!("visible"));
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
                props.insert("container_mode".to_string(), serde_json::json!("stack"));
                props.insert("display".to_string(), serde_json::json!("block"));
                props.insert(
                    "positioning_mode".to_string(),
                    serde_json::json!("absolute-children"),
                );
            }
            "grid-container" | "grid" => {
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
                props.insert("container_mode".to_string(), serde_json::json!("flex"));
                props.insert("display".to_string(), serde_json::json!("flex"));
                props.insert(
                    "responsive_mode".to_string(),
                    serde_json::json!("layout-managed"),
                );
                props.insert("flex_direction".to_string(), serde_json::json!("column"));
                props.insert("flex_wrap".to_string(), serde_json::json!("nowrap"));
                props.insert(
                    "positioning_mode".to_string(),
                    serde_json::json!("flex-only"),
                );
            }
            "div" => {
                props.remove("display");
                props.insert("text".to_string(), serde_json::json!(""));
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

    pub fn kind_for_type(element_type: &str) -> ElementKind {
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

    pub fn layout_for_type(element_type: &str) -> LayoutStyles {
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
            ElementKind::Unknown => "div",
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

        Self {
            id: element.id,
            element_type,
            x: number_property(element, "x").unwrap_or(0.0),
            y: number_property(element, "y").unwrap_or(0.0),
            width: number_property(element, "width").unwrap_or(100.0),
            height: number_property(element, "height").unwrap_or(100.0),
            rotation: number_property(element, "rotation").unwrap_or(0.0),
            properties: serde_json::to_value(&element.properties)
                .unwrap_or_else(|_| serde_json::json!({})),
        }
    }
}

pub type GeometrySnapshot = HashMap<Uuid, (f32, f32, f32, f32, f32)>;

pub fn build_page_root(width: u32, height: u32) -> UiElement {
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

pub fn page_size(page: &Page) -> (u32, u32) {
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

pub fn rotate_vector(x: f32, y: f32, rotation_deg: f32) -> (f32, f32) {
    let radians = rotation_deg.to_radians();
    let cos_v = radians.cos();
    let sin_v = radians.sin();
    (x * cos_v - y * sin_v, x * sin_v + y * cos_v)
}

pub fn rotated_bounding_box(width: f32, height: f32, rotation_deg: f32) -> (f32, f32) {
    let radians = rotation_deg.to_radians();
    let abs_cos = radians.cos().abs();
    let abs_sin = radians.sin().abs();
    (
        width * abs_cos + height * abs_sin,
        width * abs_sin + height * abs_cos,
    )
}

pub fn normalize_rotation(rotation: f32) -> f32 {
    let mut normalized = rotation;
    while normalized > 180.0 {
        normalized -= 360.0;
    }
    while normalized < -180.0 {
        normalized += 360.0;
    }
    normalized
}

pub fn clamp_to_parent_bounds(
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

pub fn transform_descendant_geometry_with_parent(
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

pub fn set_element_geometry(
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

pub fn apply_geometry_snapshot_recursive(
    elements: &mut [UiElement],
    snapshot: &GeometrySnapshot,
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

fn number_property(element: &UiElement, key: &str) -> Option<f32> {
    element
        .properties
        .get(key)
        .and_then(|v| v.as_f64())
        .map(|v| v as f32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_ui_graphs::project::Page;

    fn assert_close(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < 0.01,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn template_size_matches_known_components() {
        assert_eq!(CanvasElementData::template_size("button"), (140.0, 42.0));
        assert_eq!(
            CanvasElementData::template_size("grid-container"),
            (360.0, 240.0)
        );
        assert_eq!(CanvasElementData::template_size("unknown"), (140.0, 100.0));
    }

    #[test]
    fn template_name_seed_removes_dashes() {
        assert_eq!(CanvasElementData::name_seed("flex-container"), "flex");
        assert_eq!(CanvasElementData::name_seed("custom-card"), "customcard");
    }

    #[test]
    fn component_template_adds_identity_properties() {
        let element = CanvasElementData::from_component_template("button", 10.0, 20.0);
        assert_eq!(element.element_type, "button");
        assert_eq!(element.x, 10.0);
        assert_eq!(element.y, 20.0);
        assert_eq!(element.width, 140.0);
        assert_eq!(element.height, 42.0);
        assert_eq!(element.properties["element_type"], "button");
        assert_eq!(element.properties["display_name"], "Button");
    }

    #[test]
    fn element_kind_maps_unsupported_to_container() {
        assert_eq!(
            CanvasElementData::kind_for_type("unknown"),
            ElementKind::FlexContainer
        );
    }

    #[test]
    fn element_kind_does_not_emit_vector() {
        assert_ne!(
            CanvasElementData::kind_for_type("vector"),
            ElementKind::Unknown
        );
        assert_eq!(
            CanvasElementData::kind_for_type("vector"),
            ElementKind::FlexContainer
        );
    }

    #[test]
    fn to_ui_element_preserves_geometry_properties() {
        let element = CanvasElementData::from_component_template("label", 5.0, 7.0);
        let ui = element.to_ui_element();
        assert_eq!(ui.kind, ElementKind::Label);
        assert_eq!(ui.properties["x"], 5.0);
        assert_eq!(ui.properties["y"], 7.0);
        assert_eq!(ui.properties["element_type"], "label");
    }

    #[test]
    fn from_ui_element_reads_geometry() {
        let mut ui = CanvasElementData::from_component_template("image", 1.0, 2.0).to_ui_element();
        set_element_geometry(&mut ui, 11.0, 12.0, 200.0, 120.0, 15.0);
        let element = CanvasElementData::from_ui_element(&ui);
        assert_eq!(element.element_type, "image");
        assert_close(element.x, 11.0);
        assert_close(element.rotation, 15.0);
    }

    #[test]
    fn from_ui_element_migrates_vector_type_to_div() {
        let mut ui = CanvasElementData::from_component_template("div", 0.0, 0.0).to_ui_element();
        ui.properties
            .insert("element_type".to_string(), serde_json::json!("vector"));
        let element = CanvasElementData::from_ui_element(&ui);
        assert_eq!(element.element_type, "div");
    }

    #[test]
    fn build_page_root_sets_canvas_size() {
        let root = build_page_root(800, 600);
        assert_eq!(root.properties["width"], 800);
        assert_eq!(root.properties["height"], 600);
        assert_eq!(root.properties["element_type"], "root-canvas");
    }

    #[test]
    fn page_size_reads_root_dimensions() {
        let mut page = Page::new("Main".to_string());
        page.children.push(build_page_root(1024, 768));
        assert_eq!(page_size(&page), (1024, 768));
    }

    #[test]
    fn page_size_uses_default_without_root() {
        let page = Page::new("Main".to_string());
        assert_eq!(page_size(&page), (1920, 1080));
    }

    #[test]
    fn normalize_rotation_wraps_positive_values() {
        assert_close(normalize_rotation(270.0), -90.0);
    }

    #[test]
    fn normalize_rotation_wraps_negative_values() {
        assert_close(normalize_rotation(-270.0), 90.0);
    }

    #[test]
    fn rotate_vector_turns_point_around_origin() {
        let (x, y) = rotate_vector(10.0, 0.0, 90.0);
        assert_close(x, 0.0);
        assert_close(y, 10.0);
    }

    #[test]
    fn rotated_bounding_box_swaps_for_right_angle() {
        let (w, h) = rotated_bounding_box(30.0, 10.0, 90.0);
        assert_close(w, 10.0);
        assert_close(h, 30.0);
    }

    #[test]
    fn clamp_to_parent_bounds_keeps_child_inside_parent() {
        let (x, y, w, h) =
            clamp_to_parent_bounds(190.0, 190.0, 30.0, 30.0, 0.0, 0.0, 0.0, 200.0, 200.0, 0.0);
        assert_close(x + w, 200.0);
        assert_close(y + h, 200.0);
    }

    #[test]
    fn clamp_to_parent_bounds_scales_oversized_child() {
        let (_, _, w, h) =
            clamp_to_parent_bounds(0.0, 0.0, 500.0, 250.0, 0.0, 0.0, 0.0, 100.0, 100.0, 0.0);
        assert_close(w, 100.0);
        assert_close(h, 50.0);
    }

    #[test]
    fn transform_descendant_scales_with_parent_resize() {
        let result = transform_descendant_geometry_with_parent(
            50.0, 50.0, 20.0, 10.0, 0.0, 0.0, 0.0, 100.0, 100.0, 0.0, 0.0, 0.0, 200.0, 200.0, 0.0,
        );
        assert_close(result.0, 100.0);
        assert_close(result.1, 100.0);
        assert_close(result.2, 40.0);
        assert_close(result.3, 20.0);
    }

    #[test]
    fn transform_descendant_applies_parent_rotation_delta() {
        let result = transform_descendant_geometry_with_parent(
            10.0, 10.0, 20.0, 20.0, 15.0, 0.0, 0.0, 100.0, 100.0, 0.0, 0.0, 0.0, 100.0, 100.0, 90.0,
        );
        assert_close(result.4, 105.0);
    }

    #[test]
    fn set_element_geometry_updates_all_geometry_fields() {
        let mut ui = CanvasElementData::from_component_template("div", 0.0, 0.0).to_ui_element();
        set_element_geometry(&mut ui, 1.0, 2.0, 3.0, 4.0, 5.0);
        assert_eq!(ui.properties["x"], 1.0);
        assert_eq!(ui.properties["height"], 4.0);
        assert_eq!(ui.properties["rotation"], 5.0);
    }

    #[test]
    fn apply_geometry_snapshot_updates_matching_element() {
        let mut child =
            CanvasElementData::from_component_template("button", 0.0, 0.0).to_ui_element();
        let child_id = child.id;
        let mut root = build_page_root(500, 500);
        root.children.push(child.clone());
        let mut snapshot = GeometrySnapshot::new();
        snapshot.insert(child_id, (10.0, 20.0, 30.0, 40.0, 5.0));
        let mut changed = false;

        apply_geometry_snapshot_recursive(&mut root.children, &snapshot, &mut changed);

        assert!(changed);
        child = root.children.remove(0);
        assert_eq!(child.properties["x"], 10.0);
        assert_eq!(child.properties["rotation"], 5.0);
    }

    #[test]
    fn apply_geometry_snapshot_ignores_unknown_element() {
        let mut root = build_page_root(500, 500);
        root.children
            .push(CanvasElementData::from_component_template("button", 0.0, 0.0).to_ui_element());
        let mut snapshot = GeometrySnapshot::new();
        snapshot.insert(Uuid::new_v4(), (10.0, 20.0, 30.0, 40.0, 5.0));
        let mut changed = false;

        apply_geometry_snapshot_recursive(&mut root.children, &snapshot, &mut changed);

        assert!(!changed);
    }
}
