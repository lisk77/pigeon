use crate::{
    images::{Image, decode_notification_image},
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
    images: Arc<Mutex<HashMap<u32, Image>>>,
    event_proxy: PopupSender,
}

impl Pigeon {
    pub fn new(event_proxy: PopupSender) -> Self {
        Self {
            next_id: AtomicU32::new(1),
            notifications: Arc::new(Mutex::new(HashMap::new())),
            images: Arc::new(Mutex::new(HashMap::new())),
            event_proxy,
        }
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

    async fn notify(
        &self,
        app_name: String,
        replaces_id: u32,
        app_icon: String,
        summary: String,
        body: String,
        _actions: Vec<String>,
        hints: HashMap<String, OwnedValue>,
        expire_timeout: i32,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> u32 {
        let id = if replaces_id != 0 {
            replaces_id
        } else {
            self.next_id.fetch_add(1, Ordering::Relaxed)
        };

        match decode_notification_image(&hints, &app_icon) {
            Some(img) => {
                self.images.lock().unwrap().insert(id, img);
            }
            None => {}
        }
        let notification = Arc::new(Notification {
            id,
            replaces_id,
            app_name,
            app_icon,
            summary,
            body,
        });

        println!("\nNotification from {}", notification.app_name);
        println!("{}", notification.summary);
        println!("{}", notification.body);

        self.notifications
            .lock()
            .unwrap()
            .insert(id, notification.clone());

        let timeout = match expire_timeout {
            0 => None,
            -1 => Some(std::time::Duration::from_millis(5000)),
            milliseconds if milliseconds > 0 => {
                Some(std::time::Duration::from_millis(milliseconds as u64))
            }
            _ => None,
        };

        if let Some(timeout) = timeout {
            let notifications = Arc::clone(&self.notifications);
            let images = Arc::clone(&self.images);
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
                    images.lock().unwrap().remove(&id);
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
