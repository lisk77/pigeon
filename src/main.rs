use pigeond::{
    config::PigeonConfig,
    daemon::Pigeon,
    popup::{self, Popup, PopupEvent},
};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use zbus::connection::Builder;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Arc::new(PigeonConfig::load_default()?);

    let (event_proxy, event_source) = popup::channel();
    let dismiss_events = event_proxy.clone();
    let (dismiss_sender, dismiss_receiver) = tokio::sync::mpsc::unbounded_channel::<u32>();
    let runtime = tokio::runtime::Runtime::new()?;

    let pigeon = Pigeon::new(event_proxy, Arc::clone(&config));
    let notifications = pigeon.notifications();

    let connection = runtime.block_on(async {
        Builder::session()?
            .name("org.freedesktop.Notifications")?
            .serve_at("/org/freedesktop/Notifications", pigeon)?
            .build()
            .await
    })?;

    let dismiss_connection = connection.clone();

    runtime.spawn(async move {
        dismiss_reaction(
            dismiss_connection,
            dismiss_events,
            notifications,
            dismiss_receiver,
        )
        .await;
    });

    Popup::run(event_source, dismiss_sender, config)?;
    Ok(())
}

async fn dismiss_reaction(
    dismiss_connection: zbus::Connection,
    dismiss_events: popup::PopupSender,
    notifications: Arc<Mutex<HashMap<u32, Arc<pigeond::notification::Notification>>>>,
    mut dismiss_receiver: tokio::sync::mpsc::UnboundedReceiver<u32>,
) {
    while let Some(id) = dismiss_receiver.recv().await {
        let Some(notification) = notifications.lock().unwrap().remove(&id) else {
            continue;
        };

        let action_key = notification
            .actions
            .get_key_value("default")
            .map(|(key, _)| key.clone());

        let action_invoked = if let Some(action_key) = action_key {
            let _ = dismiss_connection
                .emit_signal(
                    None::<&str>,
                    "/org/freedesktop/Notifications",
                    "org.freedesktop.Notifications",
                    "ActionInvoked",
                    &(id, &action_key),
                )
                .await;
            true
        } else {
            false
        };

        let _ = dismiss_events.send(PopupEvent::Close(id));

        if !action_invoked {
            let _ = dismiss_connection
                .emit_signal(
                    None::<&str>,
                    "/org/freedesktop/Notifications",
                    "org.freedesktop.Notifications",
                    "NotificationClosed",
                    &(id, 2_u32),
                )
                .await;
        }
    }
}
