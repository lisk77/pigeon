use smithay_client_toolkit::{
    compositor::CompositorHandler,
    delegate_compositor, delegate_layer, delegate_output, delegate_registry, delegate_shm,
    output::{OutputHandler, OutputState},
    reexports::client::{
        Connection, QueueHandle,
        protocol::{wl_output, wl_surface},
    },
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    shell::wlr_layer::{LayerShellHandler, LayerSurface, LayerSurfaceConfigure},
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
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, layer: &LayerSurface) {
        if let Some((id, output_index)) = self.surfaces.iter().find_map(|(id, surfaces)| {
            surfaces
                .iter()
                .position(|surface| &surface.layer == layer)
                .map(|output_index| (*id, output_index))
        }) {
            self.surfaces.get_mut(&id).unwrap().remove(output_index);
            if self.surfaces[&id].is_empty() {
                self.close(id);
            } else {
                surface::restack(&self.surfaces);
            }
        }
    }

    fn configure(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        layer: &LayerSurface,
        _: LayerSurfaceConfigure,
        _: u32,
    ) {
        if let Some((id, output_index)) = self.surfaces.iter().find_map(|(id, surfaces)| {
            surfaces
                .iter()
                .position(|surface| &surface.layer == layer)
                .map(|output_index| (*id, output_index))
        }) {
            let surface = &mut self.surfaces.get_mut(&id).unwrap()[output_index];
            surface.configured = true;
            surface.draw(&mut self.pool, &mut self.fonts);
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
                    surfaces
                        .first()
                        .map(|surface| (*id, surface.notification.clone()))
                }
            })
            .collect::<Vec<_>>();

        for (id, notification) in missing {
            let surface = NotificationSurface::new(
                qh,
                &self.compositor,
                &self.layer_shell,
                notification,
                output.clone(),
            );
            self.surfaces.get_mut(&id).unwrap().push(surface);
        }
        surface::restack(&self.surfaces);
    }

    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}

    fn output_destroyed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        self.surfaces.retain(|_, surfaces| {
            surfaces.retain(|surface| surface.output != output);
            !surfaces.is_empty()
        });
        surface::restack(&self.surfaces);
    }
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
