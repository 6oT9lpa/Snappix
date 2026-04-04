use serde::{Deserialize, Serialize};

/// Единица измерения размера.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "unit", content = "value")]
pub enum SizeValue {
    Pixels(f32),
    Percent(f32),
    Fraction(f32),
    Auto,
    FitContent,
}

/// Направление Flex.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum FlexDirection {
    Row,
    Column,
}

/// Перенос элементов Flex.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum FlexWrap {
    NoWrap,
    Wrap,
    WrapReverse,
}

/// Выравнивание по главной оси.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum JustifyContent {
    FlexStart,
    Center,
    FlexEnd,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

/// Выравнивание по поперечной оси.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum AlignItems {
    FlexStart,
    Center,
    FlexEnd,
    Stretch,
    Baseline,
}

/// Выравнивание строк/столбцов.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum AlignContent {
    FlexStart,
    Center,
    FlexEnd,
    SpaceBetween,
    SpaceAround,
    Stretch,
}

/// Настройки Flex-контейнера.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct FlexLayout {
    pub direction: FlexDirection,
    pub wrap: FlexWrap,
    pub justify_content: JustifyContent,
    pub align_items: AlignItems,
    pub align_content: AlignContent,
    pub gap: Option<SizeValue>,
}

/// Настройки Grid-контейнера.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct GridLayout {
    pub columns: Vec<SizeValue>,
    pub rows: Vec<SizeValue>,
    pub gap: Option<SizeValue>,
}

/// Абсолютное позиционирование.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct AbsolutePosition {
    pub left: Option<SizeValue>,
    pub top: Option<SizeValue>,
    pub right: Option<SizeValue>,
    pub bottom: Option<SizeValue>,
}

/// Тип позиционирования.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum PositionType {
    Static,
    Relative,
    Absolute,
    Fixed,
    Sticky,
}

/// Поведение при переполнении.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Overflow {
    Visible,
    Hidden,
    Scroll,
    Auto,
}

/// Основные стили размещения.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum LayoutStyles {
    Flex(FlexLayout),
    Grid(GridLayout),
    Absolute {
        position: AbsolutePosition,
        z_index: Option<i32>,
    },
    Block {
        overflow: Option<Overflow>,
    },
    Inline {
        overflow: Option<Overflow>,
    },
}

impl Default for LayoutStyles {
    fn default() -> Self {
        Self::Block { overflow: None }
    }
}
