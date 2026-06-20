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
use super::render::{self, CARD_HEIGHT, CARD_WIDTH, FontCtx};
use crate::notification::Notification;

const TOP_MARGIN: i32 = 16;
const RIGHT_MARGIN: i32 = 16;
const CARD_GAP: i32 = 8;

pub(super) struct NotificationSurface {
    pub(super) notification: Arc<Notification>,
    pub(super) output: wl_output::WlOutput,
    pub(super) layer: LayerSurface,
    pub(super) configured: bool,
    buffer: Option<Buffer>,
}

impl NotificationSurface {
    pub(super) fn new(
        qh: &QueueHandle<Popup>,
        compositor: &CompositorState,
        layer_shell: &LayerShell,
        notification: Arc<Notification>,
        output: wl_output::WlOutput,
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
        layer.set_size(CARD_WIDTH, CARD_HEIGHT);

        Self {
            notification,
            output,
            layer,
            configured: false,
            buffer: None,
        }
    }

    pub(super) fn draw(&mut self, pool: &mut SlotPool, fonts: &mut FontCtx) {
        if !self.configured {
            return;
        }

        let stride = (CARD_WIDTH * 4) as i32;
        let (buffer, canvas) = pool
            .create_buffer(
                CARD_WIDTH as i32,
                CARD_HEIGHT as i32,
                stride,
                wl_shm::Format::Argb8888,
            )
            .expect("allocate notification buffer");

        render::render_card(canvas, CARD_WIDTH, CARD_HEIGHT, &self.notification, fonts);

        self.layer
            .wl_surface()
            .damage_buffer(0, 0, CARD_WIDTH as i32, CARD_HEIGHT as i32);
        buffer
            .attach_to(self.layer.wl_surface())
            .expect("attach notification buffer");
        self.layer.commit();

        self.buffer = Some(buffer);
    }
}

pub(super) fn restack(surfaces: &BTreeMap<u32, Vec<NotificationSurface>>) {
    for (index, surfaces) in surfaces.values().enumerate() {
        let top = TOP_MARGIN + index as i32 * (CARD_HEIGHT as i32 + CARD_GAP);
        for surface in surfaces {
            surface.layer.set_margin(top, RIGHT_MARGIN, 0, 0);
            surface.layer.commit();
        }
    }
}
