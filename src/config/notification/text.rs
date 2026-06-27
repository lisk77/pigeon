use serde::{Deserialize, Serialize};

use super::{ColorConfig, GradientDirection, deserialize_rgba_color};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct TextStyleConfig {
    pub font_size: f32,
    pub bold: bool,
    pub italic: bool,
    #[serde(
        deserialize_with = "deserialize_rgba_color",
        serialize_with = "super::serialize_rgba_color"
    )]
    pub color: ColorConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gradient_direction: Option<GradientDirection>,
    pub font_family: Option<String>,
}

impl Default for TextStyleConfig {
    fn default() -> Self {
        Self {
            font_size: 14.0,
            bold: false,
            italic: false,
            color: ColorConfig::solid([0xff, 0xff, 0xff, 0xff]),
            gradient_direction: None,
            font_family: None,
        }
    }
}
