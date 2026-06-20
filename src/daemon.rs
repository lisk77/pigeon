use crate::{
    images::{Image, decode_notification_image},
    notification::Notification,
    popup::events::{PopupEvent, PopupSender},
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
    notifications: Mutex<HashMap<u32, Arc<Notification>>>,
    images: Mutex<HashMap<u32, Image>>,
    event_proxy: PopupSender,
}

impl Pigeon {
    pub fn new(event_proxy: PopupSender) -> Self {
        Self {
            next_id: AtomicU32::new(1),
            notifications: Mutex::new(HashMap::new()),
            images: Mutex::new(HashMap::new()),
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
        _expire_timeout: i32,
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
