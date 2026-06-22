use serde::Deserialize;

use super::deserialize_rgba_color;

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct TextStyleConfig {
    pub font_size: f32,
    pub bold: bool,
    pub italic: bool,
    #[serde(deserialize_with = "deserialize_rgba_color")]
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
