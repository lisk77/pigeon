mod rules;

use serde::Deserialize;

use crate::notification::Notification;

pub use rules::{Rule, RuleAction};

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct ProfileConfig {
    pub active: String,
    pub allow_profile_override: bool,
}

impl Default for ProfileConfig {
    fn default() -> Self {
        Self {
            active: "default".into(),
            allow_profile_override: false,
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

impl Profile {
    pub fn action_for(&self, notification: &Notification) -> RuleAction {
        self.rules
            .iter()
            .find(|rule| rule.matches(notification))
            .map(|rule| rule.action)
            .unwrap_or(self.default_action)
    }
}
