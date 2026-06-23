use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct ThumbnailConfig {
    pub size: u32,
    pub gap: u32,
}

impl Default for ThumbnailConfig {
    fn default() -> Self {
        Self { size: 64, gap: 16 }
    }
}
