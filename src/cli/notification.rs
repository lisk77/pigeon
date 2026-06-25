use std::collections::HashMap;

use zbus::zvariant::Value;

#[zbus::proxy(
    interface = "org.freedesktop.Notifications",
    default_service = "org.freedesktop.Notifications",
    default_path = "/org/freedesktop/Notifications"
)]
trait Notifications {
    fn notify(
        &self,
        app_name: &str,
        replaces_id: u32,
        app_icon: &str,
        summary: &str,
        body: &str,
        actions: &[&str],
        hints: HashMap<&str, Value<'_>>,
        expire_timeout: i32,
    ) -> zbus::Result<u32>;
}

pub fn emit(summary: &str, body: &str) {
    if let Err(error) = try_emit(summary, body) {
        tracing::warn!(%error, "failed to emit notification");
    }
}

fn try_emit(summary: &str, body: &str) -> Result<(), Box<dyn std::error::Error>> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    runtime.block_on(async {
        let connection = zbus::Connection::session().await?;
        let proxy = NotificationsProxy::new(&connection).await?;
        let actions = [];
        let hints = HashMap::new();

        proxy
            .notify("pigeon", 0, "", summary, body, &actions, hints, -1)
            .await?;

        Ok(())
    })
}
