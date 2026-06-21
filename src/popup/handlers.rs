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
use super::surface::{self, NotificationSurface};

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
            self.surfaces.get_mut(&id).unwrap().remove(output_index);
            if self.surfaces[&id].is_empty() {
                self.close(qh, id);
            } else {
                let config = self.config.read().expect("config lock poisoned");
                surface::restack(&self.surfaces, &config);
            }
        }
    }

    fn configure(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
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
            {
                let surface = &mut self.surfaces.get_mut(&id).unwrap()[output_index];
                if configure.new_size.0 != 0 {
                    surface.width = configure.new_size.0;
                }
                if configure.new_size.1 != 0 {
                    surface.height = configure.new_size.1;
                }
                surface.configured = true;
                surface.draw(&mut self.pool, &mut self.fonts, &config);
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

    fn new_output(&mut self, _: &Connection, qh: &QueueHandle<Self>, output: wl_output::WlOutput) {
        let missing = self
            .surfaces
            .iter()
            .filter_map(|(id, surfaces)| {
                if surfaces.iter().any(|surface| surface.output == output) {
                    None
                } else {
                    surfaces.first().map(|surface| {
                        (
                            *id,
                            surface.notification.clone(),
                            surface.width,
                            surface.height,
                            surface.full_width,
                            surface.full_height,
                        )
                    })
                }
            })
            .collect::<Vec<_>>();

        let config = self.config.read().expect("config lock poisoned");

        for (id, notification, width, height, full_width, full_height) in missing {
            let surface = NotificationSurface::new(
                qh,
                &self.compositor,
                &self.layer_shell,
                notification,
                output.clone(),
                width,
                height,
                full_width,
                full_height,
                &config.placement,
            );
            self.surfaces.get_mut(&id).unwrap().push(surface);
        }
        surface::restack(&self.surfaces, &config);
    }

    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}

    fn output_destroyed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        let config = self.config.read().expect("config lock poisoned");
        self.surfaces.retain(|_, surfaces| {
            surfaces.retain(|surface| surface.output != output);
            !surfaces.is_empty()
        });
        surface::restack(&self.surfaces, &config);
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
