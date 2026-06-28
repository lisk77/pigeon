use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct SoundConfig {
    pub enabled: bool,
    pub file: PathBuf,
    pub volume: f32,
    pub cooldown: u64,
}

impl Default for SoundConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            file: PathBuf::new(),
            volume: 1.0,
            cooldown: 250,
        }
    }
}
