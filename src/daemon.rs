use crate::{
    config::{GeneralConfig, PigeonConfig},
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
    config: PigeonConfig,
}

impl Pigeon {
    pub fn new(event_proxy: PopupSender, config: PigeonConfig) -> Self {
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
        let id = if replaces_id != 0 {
            replaces_id
        } else {
            self.next_id.fetch_add(1, Ordering::Relaxed)
        };

        let actions: HashMap<String, String> = actions
            .chunks_exact(2)
            .map(|pair| (pair[0].clone(), pair[1].clone()))
            .collect();

        let img = decode_notification_image(&hints, &app_icon);

        let notification = Arc::new(Notification {
            id,
            replaces_id,
            app_name,
            app_icon,
            summary,
            body,
            img,
            actions,
        });

        println!("\nNotification from {}", notification.app_name);
        println!("{}", notification.summary);
        println!("{}", notification.body);
        println!("{}", notification.app_icon);
        println!("actions: {:?}", notification.actions);

        self.notifications
            .lock()
            .unwrap()
            .insert(id, notification.clone());

        let timeout = match expire_timeout {
            0 => None,
            -1 => Some(configured_timeout(&self.config.general, &hints)),
            milliseconds if milliseconds > 0 => {
                Some(std::time::Duration::from_millis(milliseconds as u64))
            }
            _ => None,
        };

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

fn configured_timeout(
    general_config: &GeneralConfig,
    hints: &HashMap<String, OwnedValue>,
) -> std::time::Duration {
    let is_low_urgency = hints
        .get("urgency")
        .and_then(|urgency| urgency.downcast_ref::<u8>().ok())
        == Some(0);

    let timeout = if is_low_urgency {
        general_config.low_timeout
    } else {
        general_config.normal_timeout
    };

    std::time::Duration::from_millis(timeout)
}
