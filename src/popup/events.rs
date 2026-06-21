use crate::notification::Notification;
use smithay_client_toolkit::reexports::calloop::channel::{self, Channel, Sender};
use std::sync::Arc;

pub enum PopupEvent {
    Show(Arc<Notification>),
    Close(u32),
    ReloadConfig,
}

pub type PopupSender = Sender<PopupEvent>;
pub type PopupReceiver = Channel<PopupEvent>;

pub fn channel() -> (PopupSender, PopupReceiver) {
    channel::channel()
}
