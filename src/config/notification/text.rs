use serde::{Deserialize, Serialize};

use super::deserialize_rgba_color;

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
    pub color: [u8; 4],
    pub font_family: Option<String>,
}

impl Default for TextStyleConfig {
    fn default() -> Self {
        Self {
            font_size: 14.0,
            bold: false,
            italic: false,
            color: [0xff, 0xff, 0xff, 0xff],
            font_family: None,
        }
    }
}
