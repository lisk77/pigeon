use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct BodyConfig {
    pub font_size: f32,
}

impl Default for BodyConfig {
    fn default() -> Self {
        Self { font_size: 14.0 }
    }
}
