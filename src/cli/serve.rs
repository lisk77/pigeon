use crate::{
    config::{PigeonConfig, SharedConfig},
    daemon::{LifecycleCommand, Pigeon, SharedQueue, refresh_queue_presentation, run_lifecycle},
    popup::{self, Popup, PopupEvent},
};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use zbus::connection::Builder;

const NOTIFICATIONS_NAME: &str = "org.freedesktop.Notifications";
const NOTIFICATIONS_PATH: &str = "/org/freedesktop/Notifications";

struct ConfigWatcher {
    _thread: std::thread::JoinHandle<()>,
}

pub fn serve() -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("starting pigeon");

    let startup_config = PigeonConfig::load_startup();
    tracing::info!(path = %startup_config.path().display(), "loaded config");
    let config: SharedConfig = Arc::new(std::sync::RwLock::new(startup_config));

    let (event_proxy, event_source) = popup::channel();
    let pigeon = Pigeon::new(event_proxy.clone(), Arc::clone(&config));
    let queue = pigeon.queue();
    let image_cache = pigeon.image_cache();
    let _config_watcher =
        match watch_config(Arc::clone(&config), event_proxy.clone(), Arc::clone(&queue)) {
            Ok(watcher) => Some(watcher),
            Err(error) => {
                tracing::warn!(%error, "config hot reload disabled");
                None
            }
        };

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()?;
    tracing::info!("started tokio lifecycle runtime");

    let (lifecycle_sender, lifecycle_receiver) =
        tokio::sync::mpsc::unbounded_channel::<LifecycleCommand>();

    let connection = runtime.block_on(async {
        tracing::info!("connecting to session D-Bus");
        let builder = Builder::session()?;

        tracing::info!(name = NOTIFICATIONS_NAME, "requesting D-Bus name");
        let builder = match builder.name(NOTIFICATIONS_NAME) {
            Ok(builder) => builder,
            Err(error) => {
                tracing::error!(
                    %error,
                    name = NOTIFICATIONS_NAME,
                    "failed to request D-Bus name"
                );
                return Err(error);
            }
        };

        let builder = builder.serve_at(NOTIFICATIONS_PATH, pigeon)?;
        let connection = builder.build().await;
        match &connection {
            Ok(_) => tracing::info!(
                name = NOTIFICATIONS_NAME,
                path = NOTIFICATIONS_PATH,
                "notification service connected"
            ),
            Err(error) => tracing::error!(%error, "failed to connect notification service"),
        }
        connection
    })?;

    runtime.spawn(run_lifecycle(
        connection,
        Arc::clone(&queue),
        image_cache,
        event_proxy,
        lifecycle_sender.clone(),
        lifecycle_receiver,
    ));
    tracing::info!("started lifecycle worker");

    tracing::info!("starting popup");
    Popup::run(event_source, config, queue, lifecycle_sender)?;
    Ok(())
}

fn watch_config(
    config: SharedConfig,
    reload_sender: popup::PopupSender,
    queue: SharedQueue,
) -> Result<ConfigWatcher, Box<dyn std::error::Error>> {
    let (ready_tx, ready_rx) = std::sync::mpsc::channel();
    let thread = std::thread::spawn(move || {
        let (event_tx, event_rx) = std::sync::mpsc::channel();
        let mut watcher: RecommendedWatcher = match notify::recommended_watcher(move |event| {
            let _ = event_tx.send(event);
        }) {
            Ok(watcher) => watcher,
            Err(error) => {
                let _ = ready_tx.send(Err(error.to_string()));
                return;
            }
        };

        let mut watched_dirs = BTreeSet::new();
        let mut watched_path = absolute_path(PigeonConfig::startup_path());
        if let Err(error) = sync_config_watches(&mut watcher, &mut watched_dirs, &watched_path) {
            let _ = ready_tx.send(Err(error.to_string()));
            return;
        }
        tracing::info!(path = %watched_path.display(), "watching config");
        let _ = ready_tx.send(Ok(()));

        while let Ok(event) = event_rx.recv() {
            match event {
                Ok(event) if is_config_change(&event, &watched_path) => {
                    while event_rx.recv_timeout(Duration::from_millis(250)).is_ok() {}

                    let reload_path = absolute_path(PigeonConfig::startup_path());
                    match PigeonConfig::load(&reload_path) {
                        Ok(new_config) => {
                            watched_path = reload_path;
                            if let Err(error) =
                                sync_config_watches(&mut watcher, &mut watched_dirs, &watched_path)
                            {
                                tracing::warn!(%error, "config watcher update failed");
                            }

                            let path = new_config.path().to_path_buf();
                            let mut config = config.write().expect("config lock poisoned");
                            *config = new_config;
                            refresh_queue_presentation(&queue, &config);
                            drop(config);
                            let _ = reload_sender.send(PopupEvent::ReloadConfig);
                            tracing::info!(path = %path.display(), "config reloaded");
                        }
                        Err(error) => {
                            tracing::warn!(
                                %error,
                                "config reload failed; keeping current config"
                            );
                        }
                    }
                }
                Ok(_) => {}
                Err(error) => tracing::warn!(%error, "config watcher error"),
            }
        }
    });

    match ready_rx.recv() {
        Ok(Ok(())) => Ok(ConfigWatcher { _thread: thread }),
        Ok(Err(error)) => Err(error.into()),
        Err(error) => Err(error.into()),
    }
}

fn is_config_change(event: &Event, config_path: &Path) -> bool {
    if !matches!(
        event.kind,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
    ) {
        return false;
    }

    let pointer_path = absolute_path(PigeonConfig::path_pointer_file());
    let pointer_parent = pointer_path.parent();
    let config_parent = config_path.parent();

    event.paths.iter().any(|path| {
        path == config_path
            || path == &pointer_path
            || pointer_parent.is_some_and(|parent| path == parent || parent.starts_with(path))
            || config_parent.is_some_and(|parent| path == parent || parent.starts_with(path))
    })
}

fn sync_config_watches(
    watcher: &mut RecommendedWatcher,
    watched_dirs: &mut BTreeSet<PathBuf>,
    config_path: &Path,
) -> notify::Result<()> {
    let desired_dirs = [
        watchable_parent(config_path),
        watchable_parent(&absolute_path(PigeonConfig::path_pointer_file())),
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();

    for dir in watched_dirs.difference(&desired_dirs) {
        watcher.unwatch(dir)?;
    }
    for dir in desired_dirs.difference(watched_dirs) {
        watcher.watch(dir, RecursiveMode::NonRecursive)?;
    }

    *watched_dirs = desired_dirs;
    Ok(())
}

fn watchable_parent(path: &Path) -> PathBuf {
    let parent = path.parent().unwrap_or(Path::new("."));
    if parent.is_dir() {
        return parent.to_path_buf();
    }

    parent
        .ancestors()
        .find(|ancestor| ancestor.is_dir())
        .unwrap_or(Path::new("."))
        .to_path_buf()
}

fn absolute_path(path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    }
}
