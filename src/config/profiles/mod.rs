mod rules;

use serde::{Deserialize, Serialize};

use crate::{
    config::{HistoryOverride, TimeoutOverride, notification::NotificationStyleOverride},
    notification::Notification,
};

pub use rules::{Rule, RuleAction};

#[derive(Debug, Deserialize, Serialize)]
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

#[derive(Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct Profile {
    pub default_action: RuleAction,
    pub rules: Vec<Rule>,
    pub notification: NotificationStyleOverride,
    pub timeouts: TimeoutOverride,
    pub history: HistoryOverride,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            default_action: RuleAction::Allow,
            rules: Vec::new(),
            notification: NotificationStyleOverride::default(),
            timeouts: TimeoutOverride::default(),
            history: HistoryOverride::default(),
        }
    }
}

impl Profile {
    pub(crate) fn is_default(&self) -> bool {
        self.default_action == RuleAction::Allow
            && self.rules.is_empty()
            && self.notification.is_empty()
            && self.timeouts.is_empty()
            && self.history.is_empty()
    }

    pub fn matching_rule(&self, notification: &Notification) -> Option<&Rule> {
        self.rules.iter().find(|rule| rule.matches(notification))
    }

    pub fn action_for(&self, notification: &Notification) -> RuleAction {
        self.matching_rule(notification)
            .and_then(|rule| rule.action)
            .unwrap_or(self.default_action)
    }
}
