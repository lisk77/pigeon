use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    sync::{Arc, Mutex},
    time::Duration,
};

use tokio::sync::mpsc::UnboundedSender;
use zbus::{Connection, object_server::SignalEmitter, zvariant::OwnedValue};

use crate::{
    config::{NotificationConfig, PigeonConfig, RuleAction, SharedConfig, TimeoutConfig},
    images::ImageCache,
    notification::{Notification, NotificationHints},
    popup::{PopupEvent, PopupSender},
};

const MAX_LIVE_NOTIFICATIONS: usize = 32;
const MAX_SHORT_TEXT_BYTES: usize = 4 * 1024;
const MAX_BODY_BYTES: usize = 64 * 1024;
const MAX_ACTION_PAIRS: usize = 32;

pub type SharedQueue = Arc<Mutex<NotificationQueue>>;

pub struct NotificationQueue {
    next_id: u32,
    pub(crate) entries: VecDeque<QueueEntry>,
}

pub(crate) struct QueueEntry {
    pub(crate) notification: Notification,
    pub(crate) generation: u64,
    pub(crate) style: NotificationConfig,
    timeout_policy: TimeoutPolicy,
    pub(crate) timeout: Option<Duration>,
    pub(crate) timer_started: bool,
}

#[derive(Clone, Copy)]
enum TimeoutPolicy {
    Never,
    Fixed(Duration),
    Configured,
}

pub enum LifecycleCommand {
    Visible { id: u32, generation: u64 },
    Dismiss { id: u32 },
    Expire { id: u32, generation: u64 },
}

pub struct Pigeon {
    queue: SharedQueue,
    image_cache: Arc<Mutex<ImageCache>>,
    event_proxy: PopupSender,
    config: SharedConfig,
}

impl NotificationQueue {
    fn new() -> Self {
        Self {
            next_id: 1,
            entries: VecDeque::new(),
        }
    }

    fn next_id(&mut self) -> u32 {
        loop {
            let id = self.next_id;
            self.next_id = self.next_id.wrapping_add(1);
            if self.next_id == 0 {
                self.next_id = 1;
            }
            if id != 0 && !self.entries.iter().any(|entry| entry.notification.id == id) {
                return id;
            }
        }
    }

    fn find_replacement(&self, replaces_id: u32, candidate: &Notification) -> Option<usize> {
        if replaces_id != 0 {
            return self
                .entries
                .iter()
                .position(|entry| entry.notification.id == replaces_id);
        }

        let tag = candidate.stack_tag()?;
        self.entries.iter().position(|entry| {
            entry.notification.stack_tag() == Some(tag)
                && same_source(&entry.notification, candidate)
        })
    }

    fn refresh_presentation(&mut self, config: &PigeonConfig) {
        for entry in &mut self.entries {
            let (_, style, timeouts, _) = config.presentation_for(&entry.notification);
            entry.style = style;
            if !entry.timer_started {
                entry.timeout = resolve(entry.timeout_policy, &timeouts, &entry.notification);
            }
        }
    }

    fn remove(&mut self, id: u32, generation: Option<u64>) -> Option<QueueEntry> {
        let index = self.entries.iter().position(|entry| {
            entry.notification.id == id
                && generation.is_none_or(|generation| entry.generation == generation)
        })?;
        self.entries.remove(index)
    }
}

impl Pigeon {
    pub fn new(event_proxy: PopupSender, config: SharedConfig) -> Self {
        Self {
            queue: Arc::new(Mutex::new(NotificationQueue::new())),
            image_cache: Arc::new(Mutex::new(ImageCache::default())),
            event_proxy,
            config,
        }
    }

    pub fn queue(&self) -> SharedQueue {
        Arc::clone(&self.queue)
    }

    pub fn image_cache(&self) -> Arc<Mutex<ImageCache>> {
        Arc::clone(&self.image_cache)
    }
}

#[zbus::interface(name = "org.freedesktop.Notifications")]
impl Pigeon {
    async fn get_server_information(&self) -> (String, String, String, String) {
        (
            "Pigeon".to_string(),
            "lisk77".to_string(),
            env!("CARGO_PKG_VERSION").to_string(),
            "1.2".to_string(),
        )
    }

    async fn get_capabilities(&self) -> Vec<String> {
        vec!["actions".to_string()]
    }

    async fn notify(
        &self,
        app_name: String,
        replaces_id: u32,
        app_icon: String,
        summary: String,
        body: String,
        actions: Vec<String>,
        mut hints: HashMap<String, OwnedValue>,
        expire: i32,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> u32 {
        let mut notification = Notification {
            id: 0,
            app_name: truncate(app_name, MAX_SHORT_TEXT_BYTES),
            app_icon: truncate(app_icon, MAX_SHORT_TEXT_BYTES),
            summary: truncate(summary, MAX_SHORT_TEXT_BYTES),
            body: truncate(body, MAX_BODY_BYTES),
            img: None,
            actions: normalize_actions(actions),
            hints: normalize_hints(&hints),
        };

        let (action, style, timeouts, history) = {
            let config = self.config.read().expect("config lock poisoned");
            config.presentation_for(&notification)
        };
        let timeout_policy = timeout_policy(expire);

        if action == RuleAction::Block {
            let id = self.queue.lock().expect("queue lock poisoned").next_id();
            tracing::info!(
                id,
                app_name = %notification.app_name,
                summary = %notification.summary,
                "blocked notification"
            );
            tracing::debug!(
                notification = format_args!("{notification:#?}"),
                "blocked notification payload"
            );
            return id;
        }

        notification.img = self
            .image_cache
            .lock()
            .expect("image cache lock poisoned")
            .thumbnail(&mut hints, &notification.app_icon, style.thumbnail.size);

        let timeout = resolve(timeout_policy, &timeouts, &notification);
        let history_entry = match crate::history::serialize(&history, &notification) {
            Ok(entry) => entry,
            Err(error) => {
                tracing::warn!(%error, "failed to serialize notification history");
                None
            }
        };
        let outcome = {
            let mut queue = self.queue.lock().expect("queue lock poisoned");
            if let Some(index) = queue.find_replacement(replaces_id, &notification) {
                let id = queue.entries[index].notification.id;
                let generation = queue.entries[index].generation.wrapping_add(1);
                notification.id = id;
                tracing::info!(
                    id,
                    generation,
                    app_name = %notification.app_name,
                    summary = %notification.summary,
                    "replaced notification"
                );
                tracing::debug!(
                    notification = format_args!("{notification:#?}"),
                    "stored notification payload"
                );
                queue.entries[index] = QueueEntry {
                    notification,
                    generation,
                    style,
                    timeout_policy,
                    timeout,
                    timer_started: false,
                };
                EnqueueOutcome::Stored(id)
            } else if queue.entries.len() >= MAX_LIVE_NOTIFICATIONS {
                let id = queue.next_id();
                tracing::warn!(
                    id,
                    app_name = %notification.app_name,
                    summary = %notification.summary,
                    max_live = MAX_LIVE_NOTIFICATIONS,
                    "rejected notification because queue is full"
                );
                tracing::debug!(
                    notification = format_args!("{notification:#?}"),
                    "rejected notification payload"
                );
                EnqueueOutcome::Rejected(id)
            } else {
                let id = queue.next_id();
                notification.id = id;
                tracing::info!(
                    id,
                    generation = 1_u64,
                    app_name = %notification.app_name,
                    summary = %notification.summary,
                    "queued notification"
                );
                tracing::debug!(
                    notification = format_args!("{notification:#?}"),
                    "stored notification payload"
                );
                queue.entries.push_back(QueueEntry {
                    notification,
                    generation: 1,
                    style,
                    timeout_policy,
                    timeout,
                    timer_started: false,
                });
                EnqueueOutcome::Stored(id)
            }
        };

        match outcome {
            EnqueueOutcome::Stored(id) => {
                if let Some(entry) = &history_entry {
                    if let Err(error) = crate::history::append_serialized(entry) {
                        tracing::warn!(%error, id, "failed to write notification history");
                    }
                }
                self.image_cache
                    .lock()
                    .expect("image cache lock poisoned")
                    .purge_dead();
                let _ = self.event_proxy.send(PopupEvent::QueueChanged);
                id
            }
            EnqueueOutcome::Rejected(id) => {
                let _ = Self::notification_closed(&emitter, id, 4).await;
                id
            }
        }
    }

    async fn close_notification(
        &self,
        id: u32,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> zbus::fdo::Result<()> {
        let removed = self
            .queue
            .lock()
            .expect("queue lock poisoned")
            .remove(id, None);
        if removed.is_some() {
            tracing::info!(id, "closed notification by request");
            self.image_cache
                .lock()
                .expect("image cache lock poisoned")
                .purge_dead();
            crate::memory::trim_free_heap_pages();
            let _ = self.event_proxy.send(PopupEvent::QueueChanged);
            Self::notification_closed(&emitter, id, 3).await?;
        }
        Ok(())
    }

    #[zbus(signal)]
    async fn notification_closed(
        emitter: &SignalEmitter<'_>,
        id: u32,
        reason: u32,
    ) -> zbus::Result<()>;
}

enum EnqueueOutcome {
    Stored(u32),
    Rejected(u32),
}

pub async fn run_lifecycle(
    connection: Connection,
    queue: SharedQueue,
    image_cache: Arc<Mutex<ImageCache>>,
    popup_events: PopupSender,
    sender: UnboundedSender<LifecycleCommand>,
    mut receiver: tokio::sync::mpsc::UnboundedReceiver<LifecycleCommand>,
) {
    while let Some(command) = receiver.recv().await {
        match command {
            LifecycleCommand::Visible { id, generation } => {
                let timeout = {
                    let mut queue = queue.lock().expect("queue lock poisoned");
                    let Some(entry) = queue.entries.iter_mut().find(|entry| {
                        entry.notification.id == id && entry.generation == generation
                    }) else {
                        continue;
                    };
                    if entry.timer_started {
                        None
                    } else {
                        entry.timer_started = true;
                        entry.timeout
                    }
                };
                if let Some(timeout) = timeout {
                    let sender = sender.clone();
                    tokio::spawn(async move {
                        tokio::time::sleep(timeout).await;
                        let _ = sender.send(LifecycleCommand::Expire { id, generation });
                    });
                }
            }
            LifecycleCommand::Dismiss { id } => {
                let removed = queue.lock().expect("queue lock poisoned").remove(id, None);
                let Some(entry) = removed else {
                    tracing::debug!(id, "ignored dismiss for unknown notification");
                    continue;
                };
                let action = entry
                    .notification
                    .actions
                    .get_key_value("default")
                    .map(|(key, _)| key.clone());
                if let Some(action) = action {
                    tracing::info!(id, action = %action, "invoking notification action");
                    let _ = connection
                        .emit_signal(
                            None::<&str>,
                            "/org/freedesktop/Notifications",
                            "org.freedesktop.Notifications",
                            "ActionInvoked",
                            &(id, action),
                        )
                        .await;
                } else {
                    tracing::info!(id, "dismissed notification without default action");
                }
                emit_closed(&connection, id, 2).await;
                image_cache
                    .lock()
                    .expect("image cache lock poisoned")
                    .purge_dead();
                crate::memory::trim_free_heap_pages();
                let _ = popup_events.send(PopupEvent::QueueChanged);
            }
            LifecycleCommand::Expire { id, generation } => {
                if queue
                    .lock()
                    .expect("queue lock poisoned")
                    .remove(id, Some(generation))
                    .is_some()
                {
                    tracing::info!(id, generation, "expired notification");
                    emit_closed(&connection, id, 1).await;
                    image_cache
                        .lock()
                        .expect("image cache lock poisoned")
                        .purge_dead();
                    crate::memory::trim_free_heap_pages();
                    let _ = popup_events.send(PopupEvent::QueueChanged);
                }
            }
        }
    }
}

pub fn refresh_queue_presentation(queue: &SharedQueue, config: &PigeonConfig) {
    queue
        .lock()
        .expect("queue lock poisoned")
        .refresh_presentation(config);
}

async fn emit_closed(connection: &Connection, id: u32, reason: u32) {
    let _ = connection
        .emit_signal(
            None::<&str>,
            "/org/freedesktop/Notifications",
            "org.freedesktop.Notifications",
            "NotificationClosed",
            &(id, reason),
        )
        .await;
}

fn timeout_policy(expire: i32) -> TimeoutPolicy {
    match expire {
        0 => TimeoutPolicy::Never,
        -1 => TimeoutPolicy::Configured,
        milliseconds if milliseconds > 0 => {
            TimeoutPolicy::Fixed(Duration::from_millis(milliseconds as u64))
        }
        _ => TimeoutPolicy::Never,
    }
}

fn resolve(
    policy: TimeoutPolicy,
    configured: &TimeoutConfig,
    notification: &Notification,
) -> Option<Duration> {
    match policy {
        TimeoutPolicy::Never => None,
        TimeoutPolicy::Fixed(timeout) => Some(timeout),
        TimeoutPolicy::Configured => {
            let milliseconds = match notification.urgency() {
                Some(0) => configured.low,
                Some(2) => configured.critical,
                _ => configured.normal,
            };
            (milliseconds != u64::MAX).then(|| Duration::from_millis(milliseconds))
        }
    }
}

fn normalize_hints(hints: &HashMap<String, OwnedValue>) -> NotificationHints {
    let mut normalized = NotificationHints::default();
    normalized.stack_tag = hints
        .get("x-dunst-stack-tag")
        .and_then(string_hint)
        .or_else(|| {
            hints
                .get("x-canonical-private-synchronous")
                .and_then(string_hint)
        });
    for (key, value) in hints {
        if matches!(
            key.as_str(),
            "image-data" | "image_data" | "icon_data" | "image-path" | "image_path"
        ) {
            continue;
        }
        match key.as_str() {
            "urgency" => normalized.urgency = value.downcast_ref::<u8>().ok(),
            "value" => normalized.progress = value.downcast_ref::<i32>().ok(),
            "transient" => normalized.transient = value.downcast_ref::<bool>().ok(),
            "resident" => normalized.resident = value.downcast_ref::<bool>().ok(),
            "desktop-entry" => normalized.desktop_entry = string_hint(value),
            "category" => normalized.category = string_hint(value),
            "x-pigeon-profile" => normalized.profile = string_hint(value),
            "x-dunst-stack-tag" | "x-canonical-private-synchronous" => {}
            _ => {
                if let Some(value) = string_hint(value) {
                    normalized.insert_string(truncate(key.clone(), MAX_SHORT_TEXT_BYTES), value);
                }
            }
        }
    }
    normalized
}

fn string_hint(value: &OwnedValue) -> Option<String> {
    <&str>::try_from(value)
        .ok()
        .map(|value| truncate(value.to_owned(), MAX_SHORT_TEXT_BYTES))
}

fn normalize_actions(actions: Vec<String>) -> BTreeMap<String, String> {
    actions
        .chunks_exact(2)
        .take(MAX_ACTION_PAIRS)
        .map(|pair| {
            (
                truncate(pair[0].clone(), MAX_SHORT_TEXT_BYTES),
                truncate(pair[1].clone(), MAX_SHORT_TEXT_BYTES),
            )
        })
        .collect()
}

fn truncate(mut value: String, limit: usize) -> String {
    if value.len() <= limit {
        return value;
    }
    let mut end = limit.saturating_sub('…'.len_utf8());
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value.truncate(end);
    value.push('…');
    value
}

fn same_source(left: &Notification, right: &Notification) -> bool {
    match (left.desktop_entry(), right.desktop_entry()) {
        (Some(left), Some(right)) => left == right,
        _ => left.app_name == right.app_name,
    }
}
