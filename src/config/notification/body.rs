use serde::Deserialize;

use super::text::TextStyleConfig;

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct BodyConfig {
    #[serde(flatten)]
    pub text: TextStyleConfig,
}

impl Default for BodyConfig {
    fn default() -> Self {
        Self {
            text: TextStyleConfig::default(),
        }
    }
}
