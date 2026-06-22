mod events;
mod handlers;
mod render;
mod surface;

pub use events::{PopupEvent, PopupReceiver, PopupSender, channel};

use std::collections::BTreeMap;
use std::sync::Arc;

use render::text::FontCtx;
use smithay_client_toolkit::{
    compositor::CompositorState,
    output::OutputState,
    reexports::client::protocol::wl_pointer,
    reexports::{
        calloop::{EventLoop, channel::Event as ChannelEvent},
        calloop_wayland_source::WaylandSource,
        client::{Connection, QueueHandle, globals::registry_queue_init},
    },
    registry::RegistryState,
    seat::SeatState,
    shell::{WaylandSurface, wlr_layer::LayerShell},
    shm::{Shm, slot::SlotPool},
};
use surface::NotificationSurface;
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    config::{PigeonConfig, SharedConfig, notification::Anchor},
    notification::Notification,
};

const MIN_VISIBLE_SURFACE_EXTENT: u32 = 48;

pub struct Popup {
    pub(in crate::popup) registry_state: RegistryState,
    pub(in crate::popup) output_state: OutputState,
    pub(in crate::popup) compositor: CompositorState,
    pub(in crate::popup) layer_shell: LayerShell,
    pub(in crate::popup) shm: Shm,
    pub(in crate::popup) pool: SlotPool,
    pub(in crate::popup) surfaces: BTreeMap<u32, Vec<NotificationSurface>>,
    pub(in crate::popup) pending: BTreeMap<u32, Arc<Notification>>,
    pub(in crate::popup) fonts: FontCtx,
    pub(in crate::popup) seat_state: SeatState,
    pub(in crate::popup) pointer: Option<wl_pointer::WlPointer>,
    pub(in crate::popup) dismiss_sender: UnboundedSender<u32>,
    pub(in crate::popup) config: SharedConfig,
}

impl Popup {
    pub fn run(
        events: PopupReceiver,
        dismiss_sender: UnboundedSender<u32>,
        config: SharedConfig,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let connection = Connection::connect_to_env()?;
        let (globals, event_queue) = registry_queue_init(&connection)?;
        let qh = event_queue.handle();

        let mut event_loop = EventLoop::<Self>::try_new()?;
        let loop_handle = event_loop.handle();
        WaylandSource::new(connection.clone(), event_queue).insert(loop_handle.clone())?;

        let compositor = CompositorState::bind(&globals, &qh)?;
        let layer_shell = LayerShell::bind(&globals, &qh)?;
        let shm = Shm::bind(&globals, &qh)?;

        let max_buffer_bytes = {
            let config = config.read().expect("config lock poisoned");
            (config.notification.max_width as usize)
                .checked_mul(config.notification.max_height as usize)
                .and_then(|pixels| pixels.checked_mul(4))
                .ok_or_else(|| std::io::Error::other("maximum card dimensions are too large"))?
        };
        let pool = SlotPool::new(max_buffer_bytes, &shm)?;
        let mut popup = Self {
            registry_state: RegistryState::new(&globals),
            output_state: OutputState::new(&globals, &qh),
            compositor,
            layer_shell,
            shm,
            pool,
            surfaces: BTreeMap::new(),
            pending: BTreeMap::new(),
            fonts: FontCtx::new(),
            seat_state: SeatState::new(&globals, &qh),
            pointer: None,
            dismiss_sender,
            config,
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
            PopupEvent::Close(id) => self.close(qh, id),
            PopupEvent::ReloadConfig => self.reload_config(),
        }
    }

    fn show(&mut self, qh: &QueueHandle<Self>, notification: Arc<Notification>) {
        let config_handle = Arc::clone(&self.config);
        let config = config_handle.read().expect("config lock poisoned");
        let id = notification.id;
        let width = config.notification.max_width;
        let height = render::card::measure_card_height(&notification, width, &mut self.fonts);

        if self
            .surfaces
            .get(&id)
            .is_some_and(|surfaces| !surfaces.is_empty())
        {
            let surfaces = self.surfaces.get_mut(&id).unwrap();
            for surface in surfaces.iter_mut() {
                surface.notification = notification.clone();
                surface.full_width = width;
                surface.full_height = height;
            }
            self.pending.remove(&id);
            self.reflow(&config);
            for surface in self.surfaces.get_mut(&id).unwrap() {
                if surface.configured {
                    surface.draw(&mut self.pool, &mut self.fonts);
                }
            }
            return;
        }

        let surfaces: Vec<NotificationSurface> = self
            .output_state
            .outputs()
            .filter_map(|output| {
                let (visible_width, visible_height) =
                    self.available_surface_size(&output, width, height, &config)?;

                Some(NotificationSurface::new(
                    qh,
                    &self.compositor,
                    &self.layer_shell,
                    notification.clone(),
                    output,
                    visible_width,
                    visible_height,
                    width,
                    height,
                    &config.notification.position,
                ))
            })
            .collect();
        if surfaces.is_empty() {
            self.pending.insert(id, Arc::clone(&notification));
        } else {
            self.pending.remove(&id);
        }
        self.surfaces.insert(id, surfaces);
        self.reflow(&config);
    }

    fn available_surface_size(
        &self,
        output: &smithay_client_toolkit::reexports::client::protocol::wl_output::WlOutput,
        width: u32,
        height: u32,
        config: &PigeonConfig,
    ) -> Option<(u32, u32)> {
        let Some((output_width, output_height)) = self.output_size(output) else {
            return Some((width, height));
        };

        let position = &config.notification.position;
        let (axis_size, leading_margin, trailing_margin, new_surface_size) = match position.anchor {
            Anchor::Top | Anchor::TopLeft | Anchor::TopRight => (
                output_height,
                position.top_margin,
                position.bottom_margin,
                height,
            ),
            Anchor::Bottom | Anchor::BottomLeft | Anchor::BottomRight => (
                output_height,
                position.bottom_margin,
                position.top_margin,
                height,
            ),
            Anchor::Left => (
                output_width,
                position.left_margin,
                position.right_margin,
                width,
            ),
            Anchor::Right => (
                output_width,
                position.right_margin,
                position.left_margin,
                width,
            ),
        };

        let mut offset = leading_margin;
        for surfaces in self.surfaces.values() {
            let Some(surface) = surfaces.iter().find(|surface| &surface.output == output) else {
                continue;
            };

            let size = match position.anchor {
                Anchor::Left | Anchor::Right => surface.width,
                _ => surface.height,
            };
            offset = offset
                .saturating_add(size)
                .saturating_add(position.notification_gap);
        }

        let available = axis_size
            .saturating_sub(trailing_margin)
            .saturating_sub(offset);
        if available < MIN_VISIBLE_SURFACE_EXTENT {
            return None;
        }

        Some(match position.anchor {
            Anchor::Left | Anchor::Right => (new_surface_size.min(available), height),
            _ => (width, new_surface_size.min(available)),
        })
    }

    fn reflow(&mut self, config: &PigeonConfig) {
        for output in self.output_state.outputs() {
            let Some((output_width, output_height)) = self.output_size(&output) else {
                continue;
            };

            let position = &config.notification.position;
            let (axis_size, leading_margin, trailing_margin) = match position.anchor {
                Anchor::Top | Anchor::TopLeft | Anchor::TopRight => {
                    (output_height, position.top_margin, position.bottom_margin)
                }
                Anchor::Bottom | Anchor::BottomLeft | Anchor::BottomRight => {
                    (output_height, position.bottom_margin, position.top_margin)
                }
                Anchor::Left => (output_width, position.left_margin, position.right_margin),
                Anchor::Right => (output_width, position.right_margin, position.left_margin),
            };

            let mut offset = leading_margin;
            for surfaces in self.surfaces.values_mut() {
                let Some(surface) = surfaces.iter_mut().find(|surface| surface.output == output)
                else {
                    continue;
                };

                let full_size = match position.anchor {
                    Anchor::Left | Anchor::Right => surface.full_width,
                    _ => surface.full_height,
                };
                let available = axis_size
                    .saturating_sub(trailing_margin)
                    .saturating_sub(offset);
                let visible_size = full_size.min(available);
                if visible_size < MIN_VISIBLE_SURFACE_EXTENT {
                    continue;
                }

                let (width, height) = match position.anchor {
                    Anchor::Left | Anchor::Right => (visible_size, surface.full_height),
                    _ => (surface.full_width, visible_size),
                };
                if surface.width != width || surface.height != height {
                    surface.width = width;
                    surface.height = height;
                    surface.layer.set_size(width, height);
                    if surface.configured {
                        surface.draw(&mut self.pool, &mut self.fonts);
                    } else {
                        surface.layer.commit();
                    }
                }

                offset = offset
                    .saturating_add(visible_size)
                    .saturating_add(position.notification_gap);
            }
        }

        surface::restack(&self.surfaces, config);
    }

    fn output_size(
        &self,
        output: &smithay_client_toolkit::reexports::client::protocol::wl_output::WlOutput,
    ) -> Option<(u32, u32)> {
        let info = self.output_state.info(output)?;
        let (width, height) = match info.logical_size {
            Some(size) => size,
            None => {
                let mode = info.modes.iter().find(|mode| mode.current)?;
                let scale = info.scale_factor.max(1);
                (mode.dimensions.0 / scale, mode.dimensions.1 / scale)
            }
        };

        Some((u32::try_from(width).ok()?, u32::try_from(height).ok()?))
    }

    pub(in crate::popup) fn close(&mut self, qh: &QueueHandle<Self>, id: u32) {
        self.surfaces.remove(&id);
        self.pending.remove(&id);
        {
            let config_handle = Arc::clone(&self.config);
            let config = config_handle.read().expect("config lock poisoned");
            self.reflow(&config);
        }
        self.show_pending(qh);
    }

    fn show_pending(&mut self, qh: &QueueHandle<Self>) {
        let notifications: Vec<_> = self.pending.values().cloned().collect();
        for notification in notifications {
            self.show(qh, notification);
        }
    }

    fn reload_config(&mut self) {
        let config_handle = Arc::clone(&self.config);
        let config = config_handle.read().expect("config lock poisoned");
        let width = config.notification.max_width;

        for surfaces in self.surfaces.values_mut() {
            for surface in surfaces {
                surface.update_position(&config.notification.position);
                surface.full_width = width;
                surface.full_height = render::card::measure_card_height(
                    &surface.notification,
                    width,
                    &mut self.fonts,
                );
            }
        }

        self.reflow(&config);
    }
}
