use serde::{Deserialize, Serialize};
use std::{env, path::PathBuf};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct HistoryConfig {
    pub enabled: bool,
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self { enabled: false }
    }
}

impl HistoryConfig {
    pub fn path() -> PathBuf {
        let state_home = env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state")))
            .unwrap_or_else(|| PathBuf::from("."));

        state_home.join("pigeon/history.jsonl")
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct HistoryOverride {
    pub enabled: Option<bool>,
}

impl HistoryOverride {
    pub fn is_empty(&self) -> bool {
        self.enabled.is_none()
    }

    pub fn apply_to(&self, base: &HistoryConfig) -> HistoryConfig {
        let mut resolved = base.clone();
        if let Some(enabled) = self.enabled {
            resolved.enabled = enabled;
        }
        resolved
    }
}
