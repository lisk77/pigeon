use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct BorderConfig {
    pub width: u32,
    #[serde(deserialize_with = "super::deserialize_rgba_color")]
    pub color: [u8; 4],
}

impl Default for BorderConfig {
    fn default() -> Self {
        Self {
            width: 1,
            color: [0x40, 0x40, 0x40, 0xff],
        }
    }
}
