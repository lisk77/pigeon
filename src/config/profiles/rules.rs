use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{
    config::{HistoryOverride, notification::NotificationStyleOverride},
    notification::Notification,
};

#[derive(Debug, Deserialize, Serialize)]
pub struct Rule {
    pub action: Option<RuleAction>,
    pub app_name: Option<String>,
    pub desktop_entry: Option<String>,
    pub summary: Option<String>,
    pub body: Option<String>,
    pub category: Option<String>,
    pub stack_tag: Option<String>,
    pub urgency: Option<u8>,
    pub transient: Option<bool>,
    pub resident: Option<bool>,
    #[serde(default)]
    pub hints: HashMap<String, String>,
    #[serde(default)]
    pub notification: NotificationStyleOverride,
    #[serde(default)]
    pub history: HistoryOverride,
}

impl Rule {
    pub fn matches(&self, notification: &Notification) -> bool {
        self.app_name
            .as_deref()
            .map_or(true, |expected| notification.app_name == expected)
            && self.desktop_entry.as_deref().map_or(true, |expected| {
                notification.desktop_entry() == Some(expected)
            })
            && self
                .summary
                .as_deref()
                .map_or(true, |expected| notification.summary == expected)
            && self
                .body
                .as_deref()
                .map_or(true, |expected| notification.body == expected)
            && self
                .category
                .as_deref()
                .map_or(true, |expected| notification.category() == Some(expected))
            && self
                .stack_tag
                .as_deref()
                .map_or(true, |expected| notification.stack_tag() == Some(expected))
            && self
                .urgency
                .map_or(true, |expected| notification.urgency() == Some(expected))
            && self
                .transient
                .map_or(true, |expected| notification.transient() == Some(expected))
            && self
                .resident
                .map_or(true, |expected| notification.resident() == Some(expected))
            && self
                .hints
                .iter()
                .all(|(key, expected)| notification.hint(key) == Some(expected.as_str()))
    }

    pub fn has_matcher(&self) -> bool {
        self.app_name.is_some()
            || self.desktop_entry.is_some()
            || self.summary.is_some()
            || self.body.is_some()
            || self.category.is_some()
            || self.stack_tag.is_some()
            || self.urgency.is_some()
            || self.transient.is_some()
            || self.resident.is_some()
            || !self.hints.is_empty()
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RuleAction {
    #[default]
    Allow,
    Block,
}
