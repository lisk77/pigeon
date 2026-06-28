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
    daemon::{LifecycleCommand, SharedQueue},
};

pub struct Popup {
    pub(in crate::popup) registry_state: RegistryState,
    pub(in crate::popup) output_state: OutputState,
    pub(in crate::popup) compositor: CompositorState,
    pub(in crate::popup) layer_shell: LayerShell,
    pub(in crate::popup) shm: Shm,
    pub(in crate::popup) queue: SharedQueue,
    pub(in crate::popup) surfaces: BTreeMap<u32, Vec<NotificationSurface>>,
    pub(in crate::popup) exiting_surfaces: Vec<NotificationSurface>,
    pub(in crate::popup) retired_frames: Vec<Frame>,
    pub(in crate::popup) fonts: Option<FontCtx>,
    pub(in crate::popup) seat_state: SeatState,
    pub(in crate::popup) pointer: Option<wl_pointer::WlPointer>,
    pub(in crate::popup) lifecycle_sender: UnboundedSender<LifecycleCommand>,
    pub(in crate::popup) config: SharedConfig,
}

impl Popup {
    pub fn run(
        events: PopupReceiver,
        config: SharedConfig,
        queue: SharedQueue,
        lifecycle_sender: UnboundedSender<LifecycleCommand>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let connection = Connection::connect_to_env()?;
        tracing::info!("connected to Wayland compositor");

        let (globals, event_queue) = registry_queue_init(&connection)?;
        let qh = event_queue.handle();

        let mut event_loop = EventLoop::<Self>::try_new()?;
        let loop_handle = event_loop.handle();
        WaylandSource::new(connection.clone(), event_queue).insert(loop_handle.clone())?;

        let compositor = CompositorState::bind(&globals, &qh)?;
        let layer_shell = LayerShell::bind(&globals, &qh)?;
        let shm = Shm::bind(&globals, &qh)?;
        tracing::info!("bound Wayland compositor, layer shell, and shared memory globals");

        let mut popup = Self {
            registry_state: RegistryState::new(&globals),
            output_state: OutputState::new(&globals, &qh),
            compositor,
            layer_shell,
            shm,
            queue,
            surfaces: BTreeMap::new(),
            exiting_surfaces: Vec::new(),
            retired_frames: Vec::new(),
            fonts: None,
            seat_state: SeatState::new(&globals, &qh),
            pointer: None,
            lifecycle_sender,
            config,
        };

        let commands_qh = qh.clone();
        loop_handle.insert_source(events, move |event, _, popup| {
            if let ChannelEvent::Msg(command) = event {
                popup.handle_command(&commands_qh, command);
            }
        })?;

        tracing::info!("popup event loop started");
        loop {
            event_loop.dispatch(None, &mut popup)?;
            popup.collect_released_frames();
        }
    }

    fn handle_command(&mut self, qh: &QueueHandle<Self>, command: PopupEvent) {
        match command {
            PopupEvent::QueueChanged => {
                let config_handle = Arc::clone(&self.config);
                let config = config_handle.read().expect("config lock poisoned");
                self.reflow(qh, &config, false);
            }
            PopupEvent::ReloadConfig => self.reload_config(qh),
        }
    }

    fn reflow(&mut self, qh: &QueueHandle<Self>, config: &PigeonConfig, force_redraw: bool) {
        let width = config.notification.max_width;
        let queue = Arc::clone(&self.queue);
        let queue = queue.lock().expect("queue lock poisoned");
        let measurements: Vec<_> = queue
            .entries
            .iter()
            .map(|entry| {
                (
                    entry.notification.id,
                    entry.generation,
                    render::card::measure_card_height(
                        &entry.notification,
                        &entry.style,
                        width,
                        self.fonts.get_or_insert_with(FontCtx::new),
                    ),
                )
            })
            .collect();
        drop(queue);
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
        let animation_duration = animation_duration(config);
        self.retain_surfaces(qh, animation_duration, |id, surface| {
            layout_ids.contains(&id) && outputs.iter().any(|output| output == &surface.output)
        });

        let mut retired_frames = Vec::new();
        let queue = Arc::clone(&self.queue);
        let queue = queue.lock().expect("queue lock poisoned");
        for output in outputs {
            for (id, generation, visible_width, visible_height, full_width, full_height) in &layout
            {
                let Some(entry) = queue
                    .entries
                    .iter()
                    .find(|entry| entry.notification.id == *id && entry.generation == *generation)
                else {
                    continue;
                };
                let surfaces = self.surfaces.entry(*id).or_default();
                if let Some(surface) = surfaces.iter_mut().find(|surface| surface.output == output)
                {
                    let notification_changed = surface.generation != *generation;
                    surface.generation = *generation;
                    surface.update_below_fullscreen(config.notification.below_fullscreen);
                    surface.update_position(&config.notification.position);
                    surface.full_width = *full_width;
                    surface.full_height = *full_height;
                    if surface.width != *visible_width || surface.height != *visible_height {
                        surface.width = *visible_width;
                        surface.height = *visible_height;
                        surface.layer.set_size(*visible_width, *visible_height);
                        if surface.configured {
                            if let Some(frame) = surface.draw(
                                &self.shm,
                                self.fonts.get_or_insert_with(FontCtx::new),
                                &entry.notification,
                                &entry.style,
                            ) {
                                retired_frames.push(frame);
                            }
                        } else {
                            surface.layer.commit();
                        }
                    } else if (notification_changed || force_redraw)
                        && surface.configured
                        && let Some(frame) = surface.draw(
                            &self.shm,
                            self.fonts.get_or_insert_with(FontCtx::new),
                            &entry.notification,
                            &entry.style,
                        )
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
                        config.notification.below_fullscreen,
                        &config.notification.position,
                    ));
                    if let Some(surface) = surfaces.last_mut()
                        && let Some(duration) = animation_duration
                    {
                        surface.start_enter(duration);
                    }
                }
            }
        }
        drop(queue);

        self.retired_frames.extend(retired_frames);

        let ordered_ids = layout.iter().map(|(id, ..)| *id).collect::<Vec<_>>();
        surface::restack(&mut self.surfaces, &ordered_ids, config);
        for (id, generation, ..) in layout {
            let _ = self
                .lifecycle_sender
                .send(LifecycleCommand::Visible { id, generation });
        }
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

    fn reload_config(&mut self, qh: &QueueHandle<Self>) {
        let config_handle = Arc::clone(&self.config);
        let config = config_handle.read().expect("config lock poisoned");
        for surfaces in self.surfaces.values_mut() {
            for surface in surfaces {
                surface.update_below_fullscreen(config.notification.below_fullscreen);
                surface.update_position(&config.notification.position);
            }
        }

        self.reflow(qh, &config, true);
    }

    pub(in crate::popup) fn collect_released_frames(&mut self) {
        self.retired_frames.retain(|frame| !frame.released());
        if self.retired_frames.is_empty()
            && self.surfaces.is_empty()
            && self.exiting_surfaces.is_empty()
            && self
                .queue
                .lock()
                .expect("queue lock poisoned")
                .entries
                .is_empty()
        {
            self.fonts = None;
            crate::memory::trim_free_heap_pages();
        }
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
        let exiting_surfaces = std::mem::take(&mut self.exiting_surfaces);
        self.retire_surfaces(exiting_surfaces);
    }

    fn retain_surfaces(
        &mut self,
        qh: &QueueHandle<Self>,
        animation_duration: Option<u32>,
        keep: impl Fn(u32, &NotificationSurface) -> bool,
    ) {
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
        for mut surface in removed {
            if let Some(duration) = animation_duration
                && surface.configured
            {
                surface.start_exit(duration);
                surface.request_animation_frame(qh);
                self.exiting_surfaces.push(surface);
            } else {
                self.retire_surface(surface);
            }
        }
    }

    pub(in crate::popup) fn remove_output(&mut self, output: &wl_output::WlOutput) {
        let mut retained = BTreeMap::new();
        let mut removed = Vec::new();
        for (id, surfaces) in std::mem::take(&mut self.surfaces) {
            let mut kept = Vec::new();
            for surface in surfaces {
                if surface.output == *output {
                    removed.push(surface);
                } else {
                    kept.push(surface);
                }
            }
            if !kept.is_empty() {
                retained.insert(id, kept);
            }
        }
        self.surfaces = retained;
        self.retire_surfaces(removed);

        let mut kept_exiting = Vec::new();
        let mut removed_exiting = Vec::new();
        for surface in std::mem::take(&mut self.exiting_surfaces) {
            if surface.output == *output {
                removed_exiting.push(surface);
            } else {
                kept_exiting.push(surface);
            }
        }
        self.exiting_surfaces = kept_exiting;
        self.retire_surfaces(removed_exiting);
    }
}

fn animation_duration(config: &PigeonConfig) -> Option<u32> {
    if !config.notification.animation.enabled {
        return None;
    }

    u32::try_from(config.notification.animation.duration).ok()
}
