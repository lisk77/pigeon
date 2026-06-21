mod rules;

use serde::Deserialize;

pub use rules::{Rule, RuleAction};

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct ProfileConfig {
    pub active: String,
    pub allow_hint_override: bool,
}

impl Default for ProfileConfig {
    fn default() -> Self {
        Self {
            active: "default".into(),
            allow_hint_override: true,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Profile {
    pub default_action: RuleAction,
    pub rules: Vec<Rule>,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            default_action: RuleAction::Allow,
            rules: Vec::new(),
        }
    }
}
