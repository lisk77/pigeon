use smithay_client_toolkit::reexports::calloop::channel::{self, Channel, Sender};

pub enum PopupEvent {
    QueueChanged,
    ReloadConfig,
}

pub type PopupSender = Sender<PopupEvent>;
pub type PopupReceiver = Channel<PopupEvent>;

pub fn channel() -> (PopupSender, PopupReceiver) {
    channel::channel()
}
