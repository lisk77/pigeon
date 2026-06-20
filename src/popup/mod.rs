pub mod events;

use std::collections::BTreeMap;

use events::{PopupEvent, PopupReceiver};
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_layer, delegate_output, delegate_registry, delegate_shm,
    output::{OutputHandler, OutputState},
    reexports::{
        calloop::{EventLoop, channel::Event as ChannelEvent},
        calloop_wayland_source::WaylandSource,
        client::{
            Connection, QueueHandle,
            globals::registry_queue_init,
            protocol::{wl_output, wl_shm, wl_surface},
        },
    },
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    shell::{
        WaylandSurface,
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
            LayerSurfaceConfigure,
        },
    },
    shm::{
        Shm, ShmHandler,
        slot::{Buffer, SlotPool},
    },
};

use crate::notification::Notification;
use std::sync::Arc;

const CARD_WIDTH: u32 = 360;
const CARD_HEIGHT: u32 = 160;
const TOP_MARGIN: i32 = 16;
const RIGHT_MARGIN: i32 = 16;
const CARD_GAP: i32 = 8;

pub struct Popup {
    registry_state: RegistryState,
    output_state: OutputState,
    compositor: CompositorState,
    layer_shell: LayerShell,
    shm: Shm,
    pool: SlotPool,
    surfaces: BTreeMap<u32, NotificationSurface>,
}

struct NotificationSurface {
    notification: Arc<Notification>,
    layer: LayerSurface,
    configured: bool,
    buffer: Option<Buffer>,
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

        if let Some(surface) = self.surfaces.get_mut(&id) {
            surface.notification = notification;
            self.draw(id, qh);
            return;
        }

        let wl_surface = self.compositor.create_surface(qh);
        let layer = self.layer_shell.create_layer_surface(
            qh,
            wl_surface,
            Layer::Overlay,
            Some("pigeond-notification"),
            None,
        );
        layer.set_anchor(Anchor::TOP | Anchor::RIGHT);
        layer.set_keyboard_interactivity(KeyboardInteractivity::None);
        layer.set_exclusive_zone(0);
        layer.set_size(CARD_WIDTH, CARD_HEIGHT);

        self.surfaces.insert(
            id,
            NotificationSurface {
                notification,
                layer,
                configured: false,
                buffer: None,
            },
        );
        self.restack();
    }

    fn close(&mut self, id: u32) {
        self.surfaces.remove(&id);
        self.restack();
    }

    fn restack(&self) {
        for (index, surface) in self.surfaces.values().enumerate() {
            let top = TOP_MARGIN + index as i32 * (CARD_HEIGHT as i32 + CARD_GAP);
            surface.layer.set_margin(top, RIGHT_MARGIN, 0, 0);
            surface.layer.commit();
        }
    }

    fn draw(&mut self, id: u32, _qh: &QueueHandle<Self>) {
        let Some(surface) = self.surfaces.get(&id) else {
            return;
        };
        if !surface.configured {
            return;
        }
        let layer = surface.layer.clone();

        let stride = (CARD_WIDTH * 4) as i32;
        let (buffer, canvas) = self
            .pool
            .create_buffer(
                CARD_WIDTH as i32,
                CARD_HEIGHT as i32,
                stride,
                wl_shm::Format::Argb8888,
            )
            .expect("allocate notification buffer");
        for pixel in canvas.chunks_exact_mut(4) {
            pixel.copy_from_slice(&[0x20, 0x20, 0x20, 0xff]);
        }

        layer
            .wl_surface()
            .damage_buffer(0, 0, CARD_WIDTH as i32, CARD_HEIGHT as i32);
        buffer
            .attach_to(layer.wl_surface())
            .expect("attach notification buffer");
        layer.commit();

        if let Some(surface) = self.surfaces.get_mut(&id) {
            surface.buffer = Some(buffer);
        }
    }
}

impl CompositorHandler for Popup {
    fn scale_factor_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: i32,
    ) {
    }

    fn transform_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: wl_output::Transform,
    ) {
    }

    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: u32) {}

    fn surface_enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }
}

impl LayerShellHandler for Popup {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, layer: &LayerSurface) {
        if let Some(id) = self
            .surfaces
            .iter()
            .find_map(|(id, surface)| (&surface.layer == layer).then_some(*id))
        {
            self.close(id);
        }
    }

    fn configure(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        layer: &LayerSurface,
        _: LayerSurfaceConfigure,
        _: u32,
    ) {
        if let Some(id) = self
            .surfaces
            .iter()
            .find_map(|(id, surface)| (&surface.layer == layer).then_some(*id))
        {
            self.surfaces.get_mut(&id).unwrap().configured = true;
            self.draw(id, qh);
        }
    }
}

impl ShmHandler for Popup {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl OutputHandler for Popup {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}

    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}

    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}

impl ProvidesRegistryState for Popup {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }

    registry_handlers![OutputState];
}

delegate_compositor!(Popup);
delegate_output!(Popup);
delegate_shm!(Popup);
delegate_layer!(Popup);
delegate_registry!(Popup);
