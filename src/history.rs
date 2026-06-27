use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::Serialize;

use crate::{
    config::HistoryConfig,
    notification::{Notification, NotificationHints},
};

pub fn serialize(
    config: &HistoryConfig,
    notification: &Notification,
) -> io::Result<Option<String>> {
    if !config.enabled {
        return Ok(None);
    }

    let entry = HistoryEntry::new(notification);
    serde_json::to_string(&entry)
        .map(|line| Some(format!("{line}\n")))
        .map_err(io::Error::other)
}

pub fn append_serialized(line: &str) -> io::Result<()> {
    let path = HistoryConfig::path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
    file.write_all(line.as_bytes())?;
    tracing::debug!(path = %path.display(), "dumped notification history entry");
    Ok(())
}

pub fn clear() -> io::Result<()> {
    let path = HistoryConfig::path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(path, "")
}

pub fn path() -> impl AsRef<Path> {
    HistoryConfig::path()
}

#[derive(Serialize)]
struct HistoryEntry<'a> {
    timestamp_unix_ms: u64,
    app_name: &'a str,
    app_icon: &'a str,
    summary: &'a str,
    body: &'a str,
    has_image: bool,
    hints: HistoryHints<'a>,
}

impl<'a> HistoryEntry<'a> {
    fn new(notification: &'a Notification) -> Self {
        Self {
            timestamp_unix_ms: timestamp_unix_ms(),
            app_name: &notification.app_name,
            app_icon: &notification.app_icon,
            summary: &notification.summary,
            body: &notification.body,
            has_image: notification.img.is_some(),
            hints: HistoryHints::new(&notification.hints),
        }
    }
}

#[derive(Serialize)]
struct HistoryHints<'a> {
    desktop_entry: Option<&'a str>,
    category: Option<&'a str>,
    urgency: Option<u8>,
    progress: Option<i32>,
    transient: Option<bool>,
    resident: Option<bool>,
    profile: Option<&'a str>,
    stack_tag: Option<&'a str>,
    strings: &'a BTreeMap<String, String>,
}

impl<'a> HistoryHints<'a> {
    fn new(hints: &'a NotificationHints) -> Self {
        Self {
            desktop_entry: hints.desktop_entry.as_deref(),
            category: hints.category.as_deref(),
            urgency: hints.urgency,
            progress: hints.progress,
            transient: hints.transient,
            resident: hints.resident,
            profile: hints.profile.as_deref(),
            stack_tag: hints.stack_tag.as_deref(),
            strings: &hints.strings,
        }
    }
}

fn timestamp_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().try_into().unwrap_or(u64::MAX))
        .unwrap_or(0)
}
