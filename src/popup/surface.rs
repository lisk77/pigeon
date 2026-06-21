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
use crate::{
    config::{PigeonConfig, PlacementConfig, Position},
    notification::Notification,
};

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
        placement: &PlacementConfig,
    ) -> Self {
        let wl_surface = compositor.create_surface(qh);
        let layer = layer_shell.create_layer_surface(
            qh,
            wl_surface,
            Layer::Overlay,
            Some("pigeon-notification"),
            Some(&output),
        );
        layer.set_anchor(anchor_for(&placement.position));
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

    pub(super) fn update_placement(&self, placement: &PlacementConfig) {
        self.layer.set_anchor(anchor_for(&placement.position));
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

fn anchor_for(position: &Position) -> Anchor {
    match position {
        Position::Top => Anchor::TOP,
        Position::TopLeft => Anchor::TOP | Anchor::LEFT,
        Position::TopRight => Anchor::TOP | Anchor::RIGHT,
        Position::Bottom => Anchor::BOTTOM,
        Position::BottomLeft => Anchor::BOTTOM | Anchor::LEFT,
        Position::BottomRight => Anchor::BOTTOM | Anchor::RIGHT,
        Position::Left => Anchor::LEFT,
        Position::Right => Anchor::RIGHT,
    }
}

pub(super) fn restack(surfaces: &BTreeMap<u32, Vec<NotificationSurface>>, config: &PigeonConfig) {
    let placement = &config.placement;
    let mut offset = match placement.position {
        Position::Top | Position::TopLeft | Position::TopRight => placement.top_margin as i32,
        Position::Bottom | Position::BottomLeft | Position::BottomRight => {
            placement.bottom_margin as i32
        }
        Position::Left => placement.left_margin as i32,
        Position::Right => placement.right_margin as i32,
    };
    let notification_gap = placement.notification_gap as i32;

    for surfaces in surfaces.values() {
        let size = surfaces
            .first()
            .map_or(0, |surface| match placement.position {
                Position::Left | Position::Right => surface.width,
                _ => surface.height,
            }) as i32;
        let margins = match placement.position {
            Position::Top => (offset, 0, 0, 0),
            Position::TopLeft => (offset, 0, 0, placement.left_margin as i32),
            Position::TopRight => (offset, placement.right_margin as i32, 0, 0),
            Position::Bottom => (0, 0, offset, 0),
            Position::BottomLeft => (0, 0, offset, placement.left_margin as i32),
            Position::BottomRight => (0, placement.right_margin as i32, offset, 0),
            Position::Left => (0, 0, 0, offset),
            Position::Right => (0, offset, 0, 0),
        };

        for surface in surfaces {
            surface
                .layer
                .set_margin(margins.0, margins.1, margins.2, margins.3);
            surface.layer.commit();
        }

        offset += size + notification_gap;
    }
}
