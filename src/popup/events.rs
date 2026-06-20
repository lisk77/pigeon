use crate::notification::Notification;
use std::sync::Arc;

pub enum PopupEvent {
    Show(Arc<Notification>),
    Close(u32),
}
