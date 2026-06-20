use pigeond::{
    daemon::Pigeon,
    popup::{Popup, events::PopupEvent},
};
use winit::event_loop::EventLoop;
use zbus::connection::Builder;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::<PopupEvent>::with_user_event().build()?;
    let event_proxy = event_loop.create_proxy();
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

    event_loop.run_app(&mut Popup::default())?;
    Ok(())
}
