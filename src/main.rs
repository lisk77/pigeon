mod daemon;

use daemon::Pigeon;
use zbus::connection::Builder;

#[tokio::main]
async fn main() -> zbus::Result<()> {
    let pigeon = Pigeon::new();

    let _ = Builder::session()?
        .name("org.freedesktop.Notifications")?
        .serve_at("/org/freedesktop/Notifications", pigeon)?
        .build()
        .await?;

    std::future::pending::<()>().await;

    Ok(())
}
