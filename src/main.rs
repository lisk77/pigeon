use pigeond::{
    daemon::Pigeon,
    popup::{self, Popup, PopupEvent},
};
use zbus::connection::Builder;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (event_proxy, event_source) = popup::channel();
    let dismiss_events = event_proxy.clone();
    let (dismiss_sender, mut dismiss_receiver) = tokio::sync::mpsc::unbounded_channel::<u32>();
    let runtime = tokio::runtime::Runtime::new()?;

    let connection = runtime.block_on(async {
        Builder::session()?
            .name("org.freedesktop.Notifications")?
            .serve_at("/org/freedesktop/Notifications", Pigeon::new(event_proxy))?
            .build()
            .await
    })?;

    let dismiss_connection = connection.clone();

    runtime.spawn(async move {
        while let Some(id) = dismiss_receiver.recv().await {
            let _ = dismiss_events.send(PopupEvent::Close(id));

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
    });

    Popup::run(event_source, dismiss_sender)?;
    Ok(())
}
