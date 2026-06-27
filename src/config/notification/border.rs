use serde::{Deserialize, Serialize};

use super::ColorConfig;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct BorderConfig {
    pub width: u32,
    #[serde(
        deserialize_with = "super::deserialize_rgba_color",
        serialize_with = "super::serialize_rgba_color"
    )]
    pub color: ColorConfig,
}

impl Default for BorderConfig {
    fn default() -> Self {
        Self {
            width: 1,
            color: ColorConfig::solid([0x40, 0x40, 0x40, 0xff]),
        }
    }
}
