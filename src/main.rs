use pigeond::{
    daemon::Pigeon,
    popup::{Popup, events},
};
use zbus::connection::Builder;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (event_proxy, event_source) = events::channel();
    let runtime = tokio::runtime::Runtime::new()?;

    let connection = runtime.block_on(async {
        Builder::session()?
            .name("org.freedesktop.Notifications")?
            .serve_at("/org/freedesktop/Notifications", Pigeon::new(event_proxy))?
            .build()
            .await
    })?;

    runtime.spawn(async move {
        let _connection = connection;
        std::future::pending::<()>().await;
    });

    Popup::run(event_source)?;
    Ok(())
}
