use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct SummaryConfig {
    pub font_size: f32,
    pub bottom_gap: f32,
}

impl Default for SummaryConfig {
    fn default() -> Self {
        Self {
            font_size: 18.0,
            bottom_gap: 8.0,
        }
    }
}
