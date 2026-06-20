pub struct Pigeon;

impl Pigeon {
    pub fn new() -> Self {
        Self
    }
}

#[zbus::interface(name = "org.freedesktop.Notifications")]
impl Pigeon {}
