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
use crate::notification::Notification;

const TOP_MARGIN: i32 = 16;
const RIGHT_MARGIN: i32 = 16;
const CARD_GAP: i32 = 8;

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

    pub(super) fn draw(&mut self, pool: &mut SlotPool, fonts: &mut FontCtx) {
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

        render::render_card(canvas, self.width, self.height, &self.notification, fonts);

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

pub(super) fn restack(surfaces: &BTreeMap<u32, Vec<NotificationSurface>>) {
    let mut top = TOP_MARGIN;

    for surfaces in surfaces.values() {
        let height = surfaces.first().map_or(0, |surface| surface.height) as i32;
        for surface in surfaces {
            surface.layer.set_margin(top, RIGHT_MARGIN, 0, 0);
            surface.layer.commit();
        }

        top += height + CARD_GAP;
    }
}
