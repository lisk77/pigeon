use std::collections::BTreeMap;
use std::sync::Arc;

use smithay_client_toolkit::{
    compositor::CompositorState,
    reexports::client::{
        QueueHandle,
        protocol::{wl_output, wl_shm},
    },
    shell::{
        WaylandSurface,
        wlr_layer::{Anchor, KeyboardInteractivity, Layer, LayerShell, LayerSurface},
    },
    shm::slot::{Buffer, SlotPool},
};

use super::Popup;
use super::render::{self, FontCtx};
use crate::{config::PigeonConfig, notification::Notification};

pub(super) struct NotificationSurface {
    pub(super) notification: Arc<Notification>,
    pub(super) output: wl_output::WlOutput,
    pub(super) layer: LayerSurface,
    pub(super) configured: bool,
    pub(super) width: u32,
    pub(super) height: u32,
    buffer: Option<Buffer>,
}

impl NotificationSurface {
    pub(super) fn new(
        qh: &QueueHandle<Popup>,
        compositor: &CompositorState,
        layer_shell: &LayerShell,
        notification: Arc<Notification>,
        output: wl_output::WlOutput,
        width: u32,
        height: u32,
    ) -> Self {
        let wl_surface = compositor.create_surface(qh);
        let layer = layer_shell.create_layer_surface(
            qh,
            wl_surface,
            Layer::Overlay,
            Some("pigeon-notification"),
            Some(&output),
        );
        layer.set_anchor(Anchor::TOP | Anchor::RIGHT);
        layer.set_keyboard_interactivity(KeyboardInteractivity::None);
        layer.set_exclusive_zone(0);
        layer.set_size(width, height);

        Self {
            notification,
            output,
            layer,
            configured: false,
            width,
            height,
            buffer: None,
        }
    }

    pub(super) fn draw(&mut self, pool: &mut SlotPool, fonts: &mut FontCtx, config: &PigeonConfig) {
        if !self.configured {
            return;
        }

        let stride = (self.width * 4) as i32;
        let (buffer, canvas) = pool
            .create_buffer(
                self.width as i32,
                self.height as i32,
                stride,
                wl_shm::Format::Argb8888,
            )
            .expect("allocate notification buffer");

        render::render_card(
            canvas,
            self.width,
            self.height,
            &self.notification,
            fonts,
            config,
        );

        self.layer
            .wl_surface()
            .damage_buffer(0, 0, self.width as i32, self.height as i32);
        buffer
            .attach_to(self.layer.wl_surface())
            .expect("attach notification buffer");
        self.layer.commit();

        self.buffer = Some(buffer);
    }
}

pub(super) fn restack(surfaces: &BTreeMap<u32, Vec<NotificationSurface>>, config: &PigeonConfig) {
    let mut top = config.placement.top_margin as i32;
    let right = config.placement.right_margin as i32;
    let notification_gap = config.placement.notification_gap as i32;

    for surfaces in surfaces.values() {
        let height = surfaces.first().map_or(0, |surface| surface.height) as i32;
        for surface in surfaces {
            surface.layer.set_margin(top, right, 0, 0);
            surface.layer.commit();
        }

        top += height + notification_gap;
    }
}
