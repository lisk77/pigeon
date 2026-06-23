mod events;
mod handlers;
mod render;
mod surface;

pub use events::{PopupEvent, PopupReceiver, PopupSender, channel};

use std::collections::{BTreeMap, BTreeSet};
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
    shm::Shm,
};
use surface::{Frame, NotificationSurface};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    config::{PigeonConfig, SharedConfig, notification::Anchor},
    daemon::SharedNotifications,
};

pub struct Popup {
    pub(in crate::popup) registry_state: RegistryState,
    pub(in crate::popup) output_state: OutputState,
    pub(in crate::popup) compositor: CompositorState,
    pub(in crate::popup) layer_shell: LayerShell,
    pub(in crate::popup) shm: Shm,
    pub(in crate::popup) notification_ids: BTreeSet<u32>,
    pub(in crate::popup) notification_store: SharedNotifications,
    pub(in crate::popup) surfaces: BTreeMap<u32, Vec<NotificationSurface>>,
    pub(in crate::popup) retired_frames: Vec<Frame>,
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
        notification_store: SharedNotifications,
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

        let mut popup = Self {
            registry_state: RegistryState::new(&globals),
            output_state: OutputState::new(&globals, &qh),
            compositor,
            layer_shell,
            shm,
            notification_ids: BTreeSet::new(),
            notification_store,
            surfaces: BTreeMap::new(),
            retired_frames: Vec::new(),
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
            popup.collect_released_frames();
        }
    }

    fn handle_command(&mut self, qh: &QueueHandle<Self>, command: PopupEvent) {
        match command {
            PopupEvent::Show(id) => self.show(qh, id),
            PopupEvent::Close(id) => self.close(qh, id),
            PopupEvent::ReloadConfig => self.reload_config(qh),
        }
    }

    fn show(&mut self, qh: &QueueHandle<Self>, id: u32) {
        if !self
            .notification_store
            .lock()
            .expect("notification store lock poisoned")
            .contains_key(&id)
        {
            return;
        }
        let config_handle = Arc::clone(&self.config);
        let config = config_handle.read().expect("config lock poisoned");
        self.notification_ids.insert(id);
        self.reflow(qh, &config);
    }

    fn reflow(&mut self, qh: &QueueHandle<Self>, config: &PigeonConfig) {
        let width = config.notification.max_width;
        let notification_store = Arc::clone(&self.notification_store);
        let notifications = notification_store
            .lock()
            .expect("notification store lock poisoned");
        self.notification_ids
            .retain(|id| notifications.contains_key(id));
        let measurements: Vec<_> = self
            .notification_ids
            .iter()
            .filter_map(|&id| {
                notifications.get(&id).map(|stored| {
                    (
                        id,
                        stored.generation,
                        render::card::measure_card_height(
                            &stored.notification,
                            width,
                            &mut self.fonts,
                        ),
                    )
                })
            })
            .collect();
        drop(notifications);
        let outputs: Vec<wl_output::WlOutput> = self.output_state.outputs().collect();
        let position = &config.notification.position;
        let Some(layout_source) = self.layout_source(&outputs, position.anchor) else {
            self.clear_surfaces();
            return;
        };
        let Some((output_width, output_height)) = self.output_size(&layout_source) else {
            self.clear_surfaces();
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

        let mut layout = Vec::new();
        let mut used = leading_margin;
        for (id, generation, height) in &measurements {
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
                *generation,
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

        let layout_ids: BTreeSet<u32> = layout.iter().map(|(id, _, _, _, _, _)| *id).collect();
        self.retain_surfaces(|id, surface| {
            layout_ids.contains(&id) && outputs.iter().any(|output| output == &surface.output)
        });

        let mut retired_frames = Vec::new();
        let notification_store = Arc::clone(&self.notification_store);
        let notifications = notification_store
            .lock()
            .expect("notification store lock poisoned");
        for output in outputs {
            for (id, generation, visible_width, visible_height, full_width, full_height) in &layout
            {
                let Some(notification) = notifications
                    .get(id)
                    .filter(|notification| notification.generation == *generation)
                    .map(|notification| &notification.notification)
                else {
                    continue;
                };
                let surfaces = self.surfaces.entry(*id).or_default();
                if let Some(surface) = surfaces.iter_mut().find(|surface| surface.output == output)
                {
                    let notification_changed = surface.generation != *generation;
                    surface.generation = *generation;
                    surface.full_width = *full_width;
                    surface.full_height = *full_height;
                    if surface.width != *visible_width || surface.height != *visible_height {
                        surface.width = *visible_width;
                        surface.height = *visible_height;
                        surface.layer.set_size(*visible_width, *visible_height);
                        if surface.configured {
                            if let Some(frame) =
                                surface.draw(&self.shm, &mut self.fonts, notification)
                            {
                                retired_frames.push(frame);
                            }
                        } else {
                            surface.layer.commit();
                        }
                    } else if notification_changed
                        && surface.configured
                        && let Some(frame) = surface.draw(&self.shm, &mut self.fonts, notification)
                    {
                        retired_frames.push(frame);
                    }
                } else {
                    surfaces.push(NotificationSurface::new(
                        qh,
                        &self.compositor,
                        &self.layer_shell,
                        *generation,
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

        self.retired_frames.extend(retired_frames);

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
        self.notification_ids.remove(&id);
        if let Some(surfaces) = self.surfaces.remove(&id) {
            self.retire_surfaces(surfaces);
        }
        {
            let config_handle = Arc::clone(&self.config);
            let config = config_handle.read().expect("config lock poisoned");
            self.reflow(qh, &config);
        }
    }

    fn reload_config(&mut self, qh: &QueueHandle<Self>) {
        let config_handle = Arc::clone(&self.config);
        let config = config_handle.read().expect("config lock poisoned");
        for surfaces in self.surfaces.values_mut() {
            for surface in surfaces {
                surface.update_position(&config.notification.position);
            }
        }

        self.reflow(qh, &config);
    }

    pub(in crate::popup) fn collect_released_frames(&mut self) {
        self.retired_frames.retain(|frame| !frame.released());
    }

    pub(in crate::popup) fn retire_surface(&mut self, mut surface: NotificationSurface) {
        if let Some(frame) = surface.take_frame() {
            self.retired_frames.push(frame);
        }
    }

    pub(in crate::popup) fn retire_surfaces(
        &mut self,
        surfaces: impl IntoIterator<Item = NotificationSurface>,
    ) {
        for surface in surfaces {
            self.retire_surface(surface);
        }
    }

    fn clear_surfaces(&mut self) {
        let surfaces = std::mem::take(&mut self.surfaces)
            .into_values()
            .flatten()
            .collect::<Vec<_>>();
        self.retire_surfaces(surfaces);
    }

    fn retain_surfaces(&mut self, keep: impl Fn(u32, &NotificationSurface) -> bool) {
        let mut retained = BTreeMap::new();
        let mut removed = Vec::new();
        for (id, surfaces) in std::mem::take(&mut self.surfaces) {
            let mut kept = Vec::new();
            for surface in surfaces {
                if keep(id, &surface) {
                    kept.push(surface);
                } else {
                    removed.push(surface);
                }
            }
            if !kept.is_empty() {
                retained.insert(id, kept);
            }
        }
        self.surfaces = retained;
        self.retire_surfaces(removed);
    }

    pub(in crate::popup) fn remove_output(&mut self, output: &wl_output::WlOutput) {
        self.retain_surfaces(|_, surface| surface.output != *output);
    }
}
