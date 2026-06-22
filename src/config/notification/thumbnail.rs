use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
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
