use crate::{
    config::{RuleAction, SharedConfig},
    images::decode_notification_image,
    notification::Notification,
    popup::{PopupEvent, PopupSender},
};
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU32, Ordering},
    },
};
use zbus::{object_server::SignalEmitter, zvariant::OwnedValue};

pub struct Pigeon {
    next_id: AtomicU32,
    notifications: Arc<Mutex<HashMap<u32, Arc<Notification>>>>,
    event_proxy: PopupSender,
    config: SharedConfig,
}

impl Pigeon {
    pub fn new(event_proxy: PopupSender, config: SharedConfig) -> Self {
        Self {
            next_id: AtomicU32::new(1),
            notifications: Arc::new(Mutex::new(HashMap::new())),
            event_proxy,
            config,
        }
    }

    pub fn notifications(&self) -> Arc<Mutex<HashMap<u32, Arc<Notification>>>> {
        Arc::clone(&self.notifications)
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
        hints: HashMap<String, OwnedValue>,
        expire_timeout: i32,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> u32 {
        let mut notification = Notification {
            id: 0,
            replaces_id,
            app_name,
            app_icon,
            summary,
            body,
            img: None,
            actions: HashMap::new(),
            hints,
            style: crate::config::NotificationConfig::default(),
        };

        let (action, style, timeouts) = {
            let config = self.config.read().expect("config lock poisoned");
            config.presentation_for(&notification)
        };
        if action == RuleAction::Block {
            return if replaces_id != 0 {
                replaces_id
            } else {
                self.next_id.fetch_add(1, Ordering::Relaxed)
            };
        }
        notification.style = style;

        notification.img = decode_notification_image(
            &notification.hints,
            &notification.app_icon,
            notification.style.thumbnail.size,
        );
        notification.hints.remove("image-data");
        notification.hints.remove("image_data");
        notification.hints.remove("icon_data");
        notification.actions = actions
            .chunks_exact(2)
            .map(|pair| (pair[0].clone(), pair[1].clone()))
            .collect();

        let timeout = match expire_timeout {
            0 => None,
            -1 => configured_timeout(&timeouts, &notification.hints),
            milliseconds if milliseconds > 0 => {
                Some(std::time::Duration::from_millis(milliseconds as u64))
            }
            _ => None,
        };

        let mut notifications = self.notifications.lock().unwrap();
        let id = if replaces_id != 0 {
            replaces_id
        } else if let Some(tag) = notification.stack_tag() {
            notifications
                .values()
                .find(|current| {
                    current.stack_tag() == Some(tag) && same_source(current, &notification)
                })
                .map(|current| current.id)
                .unwrap_or_else(|| self.next_id.fetch_add(1, Ordering::Relaxed))
        } else {
            self.next_id.fetch_add(1, Ordering::Relaxed)
        };

        notification.id = id;
        let notification = Arc::new(notification);
        notifications.insert(id, Arc::clone(&notification));
        drop(notifications);

        println!("\nNotification from {}", notification.app_name);
        println!("{}", notification.summary);
        println!("{}", notification.body);
        println!("{}", notification.app_icon);
        println!("actions: {:?}", notification.actions);

        if let Some(timeout) = timeout {
            let notifications = Arc::clone(&self.notifications);
            let event_proxy = self.event_proxy.clone();
            let emitter = emitter.to_owned();
            let timer_notification = Arc::clone(&notification);

            tokio::spawn(async move {
                tokio::time::sleep(timeout).await;

                let expired = {
                    let mut notifications = notifications.lock().unwrap();

                    match notifications.get(&id) {
                        Some(current) if Arc::ptr_eq(current, &timer_notification) => {
                            notifications.remove(&id);
                            true
                        }
                        _ => false,
                    }
                };

                if expired {
                    let _ = event_proxy.send(PopupEvent::Close(id));
                    let _ = Self::notification_closed(&emitter, id, 1).await;
                }
            });
        }

        let _ = self.event_proxy.send(PopupEvent::Show(notification));

        id
    }

    async fn close_notification(
        &self,
        id: u32,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> zbus::fdo::Result<()> {
        let removed = self.notifications.lock().unwrap().remove(&id);

        let _ = self.event_proxy.send(PopupEvent::Close(id));

        if removed.is_some() {
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

fn same_source(left: &Notification, right: &Notification) -> bool {
    match (left.desktop_entry(), right.desktop_entry()) {
        (Some(left), Some(right)) => left == right,
        _ => left.app_name == right.app_name,
    }
}

fn configured_timeout(
    timeouts: &crate::config::TimeoutConfig,
    hints: &HashMap<String, OwnedValue>,
) -> Option<std::time::Duration> {
    let timeout = match hints
        .get("urgency")
        .and_then(|urgency| urgency.downcast_ref::<u8>().ok())
    {
        Some(0) => timeouts.low_timeout,
        Some(2) => timeouts.critical_timeout,
        _ => timeouts.normal_timeout,
    };
    (timeout != u64::MAX).then(|| std::time::Duration::from_millis(timeout))
}
