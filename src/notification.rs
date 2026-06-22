use crate::{config::NotificationConfig, images::Image};
use std::collections::HashMap;
use zbus::zvariant::OwnedValue;

#[derive(Clone)]
pub struct Notification {
    pub id: u32,
    pub replaces_id: u32,
    pub app_name: String,
    pub app_icon: String,
    pub summary: String,
    pub body: String,
    pub img: Option<Image>,
    pub actions: HashMap<String, String>,
    pub hints: HashMap<String, OwnedValue>,
    pub style: NotificationConfig,
}

impl Notification {
    pub fn hint(&self, key: &str) -> Option<&OwnedValue> {
        self.hints.get(key)
    }

    pub fn urgency(&self) -> Option<u8> {
        self.hint("urgency")
            .and_then(|hint| hint.downcast_ref::<u8>().ok())
    }

    pub fn progress(&self) -> Option<i32> {
        self.hint("value")
            .and_then(|hint| hint.downcast_ref::<i32>().ok())
    }

    pub fn category(&self) -> Option<&str> {
        self.hint("category")
            .and_then(|hint| <&str>::try_from(hint).ok())
    }

    pub fn desktop_entry(&self) -> Option<&str> {
        self.hint("desktop-entry")
            .and_then(|hint| <&str>::try_from(hint).ok())
    }

    pub fn transient(&self) -> Option<bool> {
        self.hint("transient")
            .and_then(|hint| hint.downcast_ref::<bool>().ok())
    }

    pub fn resident(&self) -> Option<bool> {
        self.hint("resident")
            .and_then(|hint| hint.downcast_ref::<bool>().ok())
    }

    pub fn sound_file(&self) -> Option<&str> {
        self.hint("sound-file")
            .and_then(|hint| <&str>::try_from(hint).ok())
    }

    pub fn sound_name(&self) -> Option<&str> {
        self.hint("sound-name")
            .and_then(|hint| <&str>::try_from(hint).ok())
    }

    pub fn supress_sound(&self) -> Option<bool> {
        self.hint("supress-sound")
            .and_then(|hint| hint.downcast_ref::<bool>().ok())
    }

    pub fn profile(&self) -> Option<&str> {
        self.hint("x-pigeond-profile")
            .and_then(|hint| <&str>::try_from(hint).ok())
    }

    pub fn stack_tag(&self) -> Option<&str> {
        if let Some(tag) = self
            .hint("x-dunst-stack-tag")
            .and_then(|hint| <&str>::try_from(hint).ok())
            .or_else(|| {
                self.hint("x-canonical-private-synchronous")
                    .and_then(|hint| <&str>::try_from(hint).ok())
            })
        {
            return Some(tag);
        }
        None
    }
}
