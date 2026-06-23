use serde::{Deserialize, Serialize};

use super::text::TextStyleConfig;

#[derive(Clone, Debug, Deserialize, Serialize)]
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
