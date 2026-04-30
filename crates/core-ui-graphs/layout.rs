use serde::{Deserialize, Serialize};

/// Единица измерения размера.
#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(tag = "unit", content = "value")]
pub enum SizeValue {
    Pixels(f32),
    Percent(f32),
    Fraction(f32),
    Auto,
    FitContent,
}

impl<'de> Deserialize<'de> for SizeValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct SizeValueVisitor;

        impl<'de> serde::de::Visitor<'de> for SizeValueVisitor {
            type Value = SizeValue;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a size value")
            }

            fn visit_str<E>(self, unit: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                size_value_from_parts(unit, None).map_err(E::custom)
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let unit: String = seq
                    .next_element()?
                    .ok_or_else(|| serde::de::Error::invalid_length(0, &self))?;
                let value = seq.next_element::<f32>()?;
                size_value_from_parts(&unit, value).map_err(serde::de::Error::custom)
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let mut unit = None;
                let mut value = None;
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "unit" => unit = Some(map.next_value::<String>()?),
                        "value" => value = Some(map.next_value::<f32>()?),
                        _ => {
                            let _: serde::de::IgnoredAny = map.next_value()?;
                        }
                    }
                }
                let unit = unit.ok_or_else(|| serde::de::Error::missing_field("unit"))?;
                size_value_from_parts(&unit, value).map_err(serde::de::Error::custom)
            }
        }

        fn size_value_from_parts(unit: &str, value: Option<f32>) -> Result<SizeValue, String> {
            let normalized = unit.trim().to_ascii_lowercase().replace(['-', '_'], "");
            match normalized.as_str() {
                "pixels" => Ok(SizeValue::Pixels(value.unwrap_or(0.0))),
                "percent" => Ok(SizeValue::Percent(value.unwrap_or(0.0))),
                "fraction" => Ok(SizeValue::Fraction(value.unwrap_or(0.0))),
                "auto" => Ok(SizeValue::Auto),
                "fitcontent" => Ok(SizeValue::FitContent),
                _ => Err(format!("unknown size unit: {unit}")),
            }
        }

        deserializer.deserialize_any(SizeValueVisitor)
    }
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

#[cfg(test)]
mod tests {
    use super::SizeValue;

    #[test]
    fn size_value_reads_legacy_msgpack_tuple_variant() {
        let bytes = shared::to_msgpack(&("Pixels", 8.0f32)).expect("encode legacy shape");
        let decoded: SizeValue = shared::from_msgpack(&bytes).expect("decode legacy shape");
        assert_eq!(decoded, SizeValue::Pixels(8.0));
    }

    #[test]
    fn size_value_reads_legacy_msgpack_unit_variant() {
        let bytes = shared::to_msgpack(&"Auto").expect("encode legacy unit shape");
        let decoded: SizeValue = shared::from_msgpack(&bytes).expect("decode legacy unit shape");
        assert_eq!(decoded, SizeValue::Auto);
    }
}
