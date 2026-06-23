use smithay_client_toolkit::{
    compositor::CompositorHandler,
    delegate_compositor, delegate_layer, delegate_output, delegate_pointer, delegate_registry,
    delegate_seat, delegate_shm,
    output::{OutputHandler, OutputState},
    reexports::client::{
        Connection, QueueHandle,
        protocol::{wl_output, wl_seat, wl_surface},
    },
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        Capability, SeatHandler, SeatState,
        pointer::{PointerEvent, PointerEventKind, PointerHandler},
    },
    shell::{
        WaylandSurface,
        wlr_layer::{LayerShellHandler, LayerSurface, LayerSurfaceConfigure},
    },
    shm::{Shm, ShmHandler},
};

use super::Popup;
use super::surface;

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
    fn closed(&mut self, _: &Connection, qh: &QueueHandle<Self>, layer: &LayerSurface) {
        if let Some((id, output_index)) = self.surfaces.iter().find_map(|(id, surfaces)| {
            surfaces
                .iter()
                .position(|surface| &surface.layer == layer)
                .map(|output_index| (*id, output_index))
        }) {
            let removed = self.surfaces.get_mut(&id).unwrap().remove(output_index);
            let empty = self.surfaces[&id].is_empty();
            if empty {
                self.surfaces.remove(&id);
            }
            self.retire_surface(removed);
            let config_handle = std::sync::Arc::clone(&self.config);
            let config = config_handle.read().expect("config lock poisoned");
            self.reflow(qh, &config);
        }
    }

    fn configure(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _: u32,
    ) {
        if let Some((id, output_index)) = self.surfaces.iter().find_map(|(id, surfaces)| {
            surfaces
                .iter()
                .position(|surface| &surface.layer == layer)
                .map(|output_index| (*id, output_index))
        }) {
            let config = self.config.read().expect("config lock poisoned");
            let notification_store = std::sync::Arc::clone(&self.notification_store);
            let notifications = notification_store
                .lock()
                .expect("notification store lock poisoned");
            let Some(notification) = notifications.get(&id) else {
                drop(notifications);
                drop(config);
                self.close(qh, id);
                return;
            };
            let generation = notification.generation;
            let retired_frame = {
                let surface = &mut self.surfaces.get_mut(&id).unwrap()[output_index];
                if configure.new_size.0 != 0 {
                    surface.width = configure.new_size.0;
                }
                if configure.new_size.1 != 0 {
                    surface.height = configure.new_size.1;
                }
                surface.configured = true;
                if surface.generation == generation {
                    surface.draw(&self.shm, &mut self.fonts, &notification.notification)
                } else {
                    None
                }
            };
            if let Some(frame) = retired_frame {
                self.retired_frames.push(frame);
            }
            surface::restack(&self.surfaces, &config);
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

    fn new_output(&mut self, _: &Connection, qh: &QueueHandle<Self>, _: wl_output::WlOutput) {
        let config_handle = std::sync::Arc::clone(&self.config);
        let config = config_handle.read().expect("config lock poisoned");
        self.reflow(qh, &config);
    }

    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}

    fn output_destroyed(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        let config_handle = std::sync::Arc::clone(&self.config);
        let config = config_handle.read().expect("config lock poisoned");
        self.remove_output(&output);
        self.reflow(qh, &config);
    }
}

impl SeatHandler for Popup {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }

    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}

    fn new_capability(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Pointer && self.pointer.is_none() {
            self.pointer = self.seat_state.get_pointer(qh, &seat).ok();
        }
    }

    fn remove_capability(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Pointer {
            self.pointer.take();
        }
    }

    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl PointerHandler for Popup {
    fn pointer_frame(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &smithay_client_toolkit::reexports::client::protocol::wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        for event in events {
            if !matches!(event.kind, PointerEventKind::Press { .. }) {
                continue;
            }

            if let Some((id, _)) = self.surfaces.iter().find_map(|(id, surfaces)| {
                surfaces
                    .iter()
                    .find(|surface| surface.layer.wl_surface() == &event.surface)
                    .map(|surface| (*id, surface))
            }) {
                let _ = self.dismiss_sender.send(id);
            }
        }
    }
}

impl ProvidesRegistryState for Popup {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }

    registry_handlers![OutputState, SeatState];
}

delegate_compositor!(Popup);
delegate_output!(Popup);
delegate_seat!(Popup);
delegate_pointer!(Popup);
delegate_shm!(Popup);
delegate_layer!(Popup);
delegate_registry!(Popup);
