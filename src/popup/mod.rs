mod events;
mod handlers;
mod render;
mod surface;

pub use events::{PopupEvent, PopupReceiver, PopupSender, channel};

use std::collections::BTreeMap;
use std::sync::Arc;

use render::{CARD_HEIGHT, CARD_WIDTH, FontCtx};
use smithay_client_toolkit::{
    compositor::CompositorState,
    output::OutputState,
    reexports::{
        calloop::{EventLoop, channel::Event as ChannelEvent},
        calloop_wayland_source::WaylandSource,
        client::{Connection, QueueHandle, globals::registry_queue_init},
    },
    registry::RegistryState,
    shell::wlr_layer::LayerShell,
    shm::{Shm, slot::SlotPool},
};
use surface::NotificationSurface;

use crate::notification::Notification;

pub struct Popup {
    pub(in crate::popup) registry_state: RegistryState,
    pub(in crate::popup) output_state: OutputState,
    pub(in crate::popup) compositor: CompositorState,
    pub(in crate::popup) layer_shell: LayerShell,
    pub(in crate::popup) shm: Shm,
    pub(in crate::popup) pool: SlotPool,
    pub(in crate::popup) surfaces: BTreeMap<u32, Vec<NotificationSurface>>,
    pub(in crate::popup) fonts: FontCtx,
}

impl Popup {
    pub fn run(events: PopupReceiver) -> Result<(), Box<dyn std::error::Error>> {
        let connection = Connection::connect_to_env()?;
        let (globals, event_queue) = registry_queue_init(&connection)?;
        let qh = event_queue.handle();

        let mut event_loop = EventLoop::<Self>::try_new()?;
        let loop_handle = event_loop.handle();
        WaylandSource::new(connection.clone(), event_queue).insert(loop_handle.clone())?;

        let compositor = CompositorState::bind(&globals, &qh)?;
        let layer_shell = LayerShell::bind(&globals, &qh)?;
        let shm = Shm::bind(&globals, &qh)?;

        let pool = SlotPool::new((CARD_WIDTH * CARD_HEIGHT * 4) as usize, &shm)?;
        let mut popup = Self {
            registry_state: RegistryState::new(&globals),
            output_state: OutputState::new(&globals, &qh),
            compositor,
            layer_shell,
            shm,
            pool,
            surfaces: BTreeMap::new(),
            fonts: FontCtx::new(),
        };

        let commands_qh = qh.clone();
        loop_handle.insert_source(events, move |event, _, popup| {
            if let ChannelEvent::Msg(command) = event {
                popup.handle_command(&commands_qh, command);
            }
        })?;

        loop {
            event_loop.dispatch(None, &mut popup)?;
        }
    }

    fn handle_command(&mut self, qh: &QueueHandle<Self>, command: PopupEvent) {
        match command {
            PopupEvent::Show(notification) => self.show(qh, notification),
            PopupEvent::Close(id) => self.close(id),
        }
    }

    fn show(&mut self, qh: &QueueHandle<Self>, notification: Arc<Notification>) {
        let id = notification.id;

        if let Some(surfaces) = self.surfaces.get_mut(&id) {
            for surface in surfaces.iter_mut() {
                surface.notification = notification.clone();
                surface.draw(&mut self.pool, &mut self.fonts);
            }
            return;
        }

        let surfaces = self
            .output_state
            .outputs()
            .map(|output| {
                NotificationSurface::new(
                    qh,
                    &self.compositor,
                    &self.layer_shell,
                    notification.clone(),
                    output,
                )
            })
            .collect();
        self.surfaces.insert(id, surfaces);
        surface::restack(&self.surfaces);
    }

    pub(in crate::popup) fn close(&mut self, id: u32) {
        self.surfaces.remove(&id);
        surface::restack(&self.surfaces);
    }
}
