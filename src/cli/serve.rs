use crate::{
    config::{PigeonConfig, SharedConfig},
    daemon::Pigeon,
    popup::{self, Popup, PopupEvent},
};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::{path::Path, sync::Arc, time::Duration};
use zbus::connection::Builder;

pub fn serve() -> Result<(), Box<dyn std::error::Error>> {
    let config: SharedConfig = Arc::new(std::sync::RwLock::new(PigeonConfig::load_startup()));

    let (event_proxy, event_source) = popup::channel();
    let _config_watcher = match watch_config(Arc::clone(&config), event_proxy.clone()) {
        Ok(watcher) => Some(watcher),
        Err(error) => {
            eprintln!("config hot reload disabled: {error}");
            None
        }
    };
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
    let dismiss_notifications = Arc::clone(&notifications);

    runtime.spawn(async move {
        dismiss_reaction(
            dismiss_connection,
            dismiss_events,
            dismiss_notifications,
            dismiss_receiver,
        )
        .await;
    });

    Popup::run(event_source, dismiss_sender, config, notifications)?;
    Ok(())
}

fn watch_config(
    config: SharedConfig,
    reload_sender: popup::PopupSender,
) -> Result<RecommendedWatcher, Box<dyn std::error::Error>> {
    let config_path = PigeonConfig::default_path();
    let watched_path = if config_path.is_absolute() {
        config_path
    } else {
        std::env::current_dir()?.join(config_path)
    };
    let config_dir = watched_path
        .parent()
        .unwrap_or(Path::new("."))
        .to_path_buf();
    std::fs::create_dir_all(&config_dir)?;

    let (reload_tx, reload_rx) = std::sync::mpsc::channel();
    let reload_path = watched_path.clone();
    std::thread::spawn(move || {
        while reload_rx.recv().is_ok() {
            while reload_rx.recv_timeout(Duration::from_millis(250)).is_ok() {}

            match PigeonConfig::load(&reload_path) {
                Ok(new_config) => {
                    *config.write().expect("config lock poisoned") = new_config;
                    let _ = reload_sender.send(PopupEvent::ReloadConfig);
                    eprintln!("config reloaded");
                }
                Err(error) => eprintln!("config reload failed; keeping current config: {error}"),
            }
        }
    });

    let mut watcher = notify::recommended_watcher(move |event| match event {
        Ok(event) if is_config_change(&event, &watched_path) => {
            let _ = reload_tx.send(());
        }
        Ok(_) => {}
        Err(error) => eprintln!("config watcher error: {error}"),
    })?;
    watcher.watch(&config_dir, RecursiveMode::NonRecursive)?;

    Ok(watcher)
}

fn is_config_change(event: &Event, config_path: &Path) -> bool {
    matches!(
        event.kind,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
    ) && event.paths.iter().any(|path| path == config_path)
}

async fn dismiss_reaction(
    dismiss_connection: zbus::Connection,
    dismiss_events: popup::PopupSender,
    notifications: crate::daemon::SharedNotifications,
    mut dismiss_receiver: tokio::sync::mpsc::UnboundedReceiver<u32>,
) {
    while let Some(id) = dismiss_receiver.recv().await {
        let Some(notification) = notifications.lock().unwrap().remove(&id) else {
            continue;
        };

        let action_key = notification
            .notification
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
