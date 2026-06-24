use crate::{
    config::{PigeonConfig, SharedConfig},
    daemon::{LifecycleCommand, Pigeon, SharedQueue, refresh_queue_presentation, run_lifecycle},
    popup::{self, Popup, PopupEvent},
};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::{path::Path, sync::Arc, time::Duration};
use zbus::connection::Builder;

pub fn serve() -> Result<(), Box<dyn std::error::Error>> {
    let config: SharedConfig = Arc::new(std::sync::RwLock::new(PigeonConfig::load_startup()));
    let (event_proxy, event_source) = popup::channel();
    let pigeon = Pigeon::new(event_proxy.clone(), Arc::clone(&config));
    let queue = pigeon.queue();
    let image_cache = pigeon.image_cache();
    let _config_watcher =
        match watch_config(Arc::clone(&config), event_proxy.clone(), Arc::clone(&queue)) {
            Ok(watcher) => Some(watcher),
            Err(error) => {
                eprintln!("config hot reload disabled: {error}");
                None
            }
        };

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()?;
    let (lifecycle_sender, lifecycle_receiver) =
        tokio::sync::mpsc::unbounded_channel::<LifecycleCommand>();
    let connection = runtime.block_on(async {
        Builder::session()?
            .name("org.freedesktop.Notifications")?
            .serve_at("/org/freedesktop/Notifications", pigeon)?
            .build()
            .await
    })?;

    runtime.spawn(run_lifecycle(
        connection,
        Arc::clone(&queue),
        image_cache,
        event_proxy,
        lifecycle_sender.clone(),
        lifecycle_receiver,
    ));

    Popup::run(event_source, config, queue, lifecycle_sender)?;
    Ok(())
}

fn watch_config(
    config: SharedConfig,
    reload_sender: popup::PopupSender,
    queue: SharedQueue,
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
                    let mut config = config.write().expect("config lock poisoned");
                    *config = new_config;
                    refresh_queue_presentation(&queue, &config);
                    drop(config);
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
