use serde::{Deserialize, Serialize};

use super::text::TextStyleConfig;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct SummaryConfig {
    #[serde(flatten)]
    pub text: TextStyleConfig,
    pub bottom_gap: f32,
}

impl Default for SummaryConfig {
    fn default() -> Self {
        Self {
            text: TextStyleConfig {
                font_size: 18.0,
                bold: true,
                ..TextStyleConfig::default()
            },
            bottom_gap: 8.0,
        }
    }
}
