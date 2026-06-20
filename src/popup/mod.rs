pub mod events;

use super::notification::Notification;
use events::PopupEvent;

use std::sync::Arc;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::ActiveEventLoop,
    window::{Window, WindowId},
};

#[derive(Default)]
pub struct Popup {
    window: Option<Window>,
    notifications: Vec<Arc<Notification>>,
}

impl ApplicationHandler<PopupEvent> for Popup {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.window = Some(
            event_loop
                .create_window(Window::default_attributes())
                .unwrap(),
        );
    }

    fn user_event(&mut self, _: &ActiveEventLoop, event: PopupEvent) {
        match event {
            PopupEvent::Show(notification) => {
                self.notifications.retain(|item| item.id != notification.id);
                self.notifications.push(notification);
            }
            PopupEvent::Close(id) => self.notifications.retain(|item| item.id != id),
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                self.window.as_ref().unwrap().request_redraw();
            }
            _ => (),
        }
    }
}
