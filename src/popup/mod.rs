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
    reexports::client::protocol::{wl_output, wl_pointer},
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

pub struct Popup {
    pub(in crate::popup) registry_state: RegistryState,
    pub(in crate::popup) output_state: OutputState,
    pub(in crate::popup) compositor: CompositorState,
    pub(in crate::popup) layer_shell: LayerShell,
    pub(in crate::popup) shm: Shm,
    pub(in crate::popup) pool: SlotPool,
    pub(in crate::popup) notifications: BTreeMap<u32, Arc<Notification>>,
    pub(in crate::popup) surfaces: BTreeMap<u32, Vec<NotificationSurface>>,
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
            notifications: BTreeMap::new(),
            surfaces: BTreeMap::new(),
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
            PopupEvent::ReloadConfig => self.reload_config(qh),
        }
    }

    fn show(&mut self, qh: &QueueHandle<Self>, notification: Arc<Notification>) {
        let config_handle = Arc::clone(&self.config);
        let config = config_handle.read().expect("config lock poisoned");
        self.notifications.insert(notification.id, notification);
        self.reflow(qh, &config);
    }

    fn reflow(&mut self, qh: &QueueHandle<Self>, config: &PigeonConfig) {
        let width = config.notification.max_width;
        let notifications: Vec<_> = self
            .notifications
            .iter()
            .map(|(&id, notification)| {
                (
                    id,
                    Arc::clone(notification),
                    render::card::measure_card_height(notification, width, &mut self.fonts),
                )
            })
            .collect();
        let outputs: Vec<wl_output::WlOutput> = self.output_state.outputs().collect();
        let position = &config.notification.position;
        let Some(layout_source) = self.layout_source(&outputs, position.anchor) else {
            self.surfaces.clear();
            return;
        };
        let Some((output_width, output_height)) = self.output_size(&layout_source) else {
            self.surfaces.clear();
            return;
        };

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

        // This is the one central visible buffer. Every output receives this
        // exact layout rather than calculating its own independent stack.
        let mut layout = Vec::new();
        let mut used = leading_margin;
        for (id, notification, height) in &notifications {
            let full_size = match position.anchor {
                Anchor::Left | Anchor::Right => width,
                _ => *height,
            };
            let available = axis_size
                .saturating_sub(trailing_margin)
                .saturating_sub(used);
            if available == 0 {
                break;
            }

            let (visible_width, visible_height) = match position.anchor {
                Anchor::Left | Anchor::Right => (width.min(available), *height),
                _ => (width, (*height).min(available)),
            };
            layout.push((
                *id,
                Arc::clone(notification),
                visible_width,
                visible_height,
                width,
                *height,
            ));

            if full_size >= available {
                break;
            }
            used = used
                .saturating_add(full_size)
                .saturating_add(position.notification_gap);
        }

        self.surfaces.retain(|id, surfaces| {
            surfaces.retain(|surface| {
                outputs.iter().any(|output| output == &surface.output)
                    && layout
                        .iter()
                        .any(|(layout_id, _, _, _, _, _)| layout_id == id)
            });
            !surfaces.is_empty()
        });

        for output in outputs {
            for (id, notification, visible_width, visible_height, full_width, full_height) in
                &layout
            {
                let surfaces = self.surfaces.entry(*id).or_default();
                if let Some(surface) = surfaces.iter_mut().find(|surface| surface.output == output)
                {
                    let notification_changed = !Arc::ptr_eq(&surface.notification, notification);
                    surface.notification = Arc::clone(notification);
                    surface.full_width = *full_width;
                    surface.full_height = *full_height;
                    if surface.width != *visible_width || surface.height != *visible_height {
                        surface.width = *visible_width;
                        surface.height = *visible_height;
                        surface.layer.set_size(*visible_width, *visible_height);
                        if surface.configured {
                            surface.draw(&mut self.pool, &mut self.fonts);
                        } else {
                            surface.layer.commit();
                        }
                    } else if notification_changed && surface.configured {
                        surface.draw(&mut self.pool, &mut self.fonts);
                    }
                } else {
                    surfaces.push(NotificationSurface::new(
                        qh,
                        &self.compositor,
                        &self.layer_shell,
                        Arc::clone(notification),
                        output.clone(),
                        *visible_width,
                        *visible_height,
                        *full_width,
                        *full_height,
                        &config.notification.position,
                    ));
                }
            }
        }

        surface::restack(&self.surfaces, config);
    }

    fn layout_source(
        &self,
        outputs: &[wl_output::WlOutput],
        anchor: Anchor,
    ) -> Option<wl_output::WlOutput> {
        outputs
            .iter()
            .filter_map(|output| {
                self.output_size(output).map(|(width, height)| {
                    let extent = match anchor {
                        Anchor::Left | Anchor::Right => width,
                        _ => height,
                    };
                    (extent, output.clone())
                })
            })
            .max_by_key(|(extent, _)| *extent)
            .map(|(_, output)| output)
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
        self.notifications.remove(&id);
        self.surfaces.remove(&id);
        {
            let config_handle = Arc::clone(&self.config);
            let config = config_handle.read().expect("config lock poisoned");
            self.reflow(qh, &config);
        }
    }

    fn reload_config(&mut self, qh: &QueueHandle<Self>) {
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

        self.reflow(qh, &config);
    }
}
