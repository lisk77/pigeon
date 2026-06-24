use std::{collections::BTreeMap, sync::Arc};

use crate::images::Image;

const MAX_STRING_HINTS: usize = 64;

#[derive(Clone, Default)]
pub struct NotificationHints {
    pub desktop_entry: Option<String>,
    pub category: Option<String>,
    pub urgency: Option<u8>,
    pub progress: Option<i32>,
    pub transient: Option<bool>,
    pub resident: Option<bool>,
    pub profile: Option<String>,
    pub stack_tag: Option<String>,
    pub strings: BTreeMap<String, String>,
}

impl NotificationHints {
    pub fn insert_string(&mut self, key: String, value: String) {
        if self.strings.len() < MAX_STRING_HINTS || self.strings.contains_key(&key) {
            self.strings.insert(key, value);
        }
    }

    pub fn hint(&self, key: &str) -> Option<&str> {
        match key {
            "desktop-entry" => self.desktop_entry.as_deref(),
            "category" => self.category.as_deref(),
            "x-pigeon-profile" => self.profile.as_deref(),
            "x-dunst-stack-tag" | "x-canonical-private-synchronous" => self.stack_tag.as_deref(),
            _ => self.strings.get(key).map(String::as_str),
        }
    }
}

pub struct Notification {
    pub id: u32,
    pub app_name: String,
    pub app_icon: String,
    pub summary: String,
    pub body: String,
    pub img: Option<Arc<Image>>,
    pub actions: BTreeMap<String, String>,
    pub hints: NotificationHints,
}

impl Notification {
    pub fn hint(&self, key: &str) -> Option<&str> {
        self.hints.hint(key)
    }

    pub fn urgency(&self) -> Option<u8> {
        self.hints.urgency
    }

    pub fn progress(&self) -> Option<i32> {
        self.hints.progress
    }

    pub fn category(&self) -> Option<&str> {
        self.hints.category.as_deref()
    }

    pub fn desktop_entry(&self) -> Option<&str> {
        self.hints.desktop_entry.as_deref()
    }

    pub fn transient(&self) -> Option<bool> {
        self.hints.transient
    }

    pub fn resident(&self) -> Option<bool> {
        self.hints.resident
    }

    pub fn profile(&self) -> Option<&str> {
        self.hints.profile.as_deref()
    }

    pub fn stack_tag(&self) -> Option<&str> {
        self.hints.stack_tag.as_deref()
    }
}
