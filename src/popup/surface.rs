use std::collections::BTreeMap;

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
    shm::{
        Shm,
        slot::{Buffer, SlotPool},
    },
};

use super::Popup;
use super::render::{self, text::FontCtx};
use crate::{
    config::{
        NotificationConfig, PigeonConfig,
        notification::{Anchor as PositionAnchor, PositionConfig},
    },
    notification::Notification,
};

pub(super) struct NotificationSurface {
    pub(super) generation: u64,
    pub(super) output: wl_output::WlOutput,
    pub(super) layer: LayerSurface,
    pub(super) configured: bool,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) full_width: u32,
    pub(super) full_height: u32,
    frame: Option<Frame>,
}

pub(super) struct Frame {
    buffer: Buffer,
    _pool: SlotPool,
}

impl Frame {
    pub(super) fn released(&self) -> bool {
        !self.buffer.slot().has_active_buffers()
    }
}

impl NotificationSurface {
    pub(super) fn new(
        qh: &QueueHandle<Popup>,
        compositor: &CompositorState,
        layer_shell: &LayerShell,
        generation: u64,
        output: wl_output::WlOutput,
        width: u32,
        height: u32,
        full_width: u32,
        full_height: u32,
        position: &PositionConfig,
    ) -> Self {
        let wl_surface = compositor.create_surface(qh);
        let layer = layer_shell.create_layer_surface(
            qh,
            wl_surface,
            Layer::Overlay,
            Some("pigeon-notification"),
            Some(&output),
        );
        layer.set_anchor(anchor_for(&position.anchor));
        layer.set_keyboard_interactivity(KeyboardInteractivity::None);
        layer.set_exclusive_zone(0);
        layer.set_size(width, height);

        Self {
            generation,
            output,
            layer,
            configured: false,
            width,
            height,
            full_width,
            full_height,
            frame: None,
        }
    }

    pub(super) fn update_position(&self, position: &PositionConfig) {
        self.layer.set_anchor(anchor_for(&position.anchor));
    }

    pub(super) fn draw(
        &mut self,
        shm: &Shm,
        fonts: &mut FontCtx,
        notification: &Notification,
        style: &NotificationConfig,
    ) -> Option<Frame> {
        if !self.configured {
            return None;
        }

        let stride = self
            .width
            .checked_mul(4)
            .expect("notification buffer stride overflow");
        let bytes = (stride as usize)
            .checked_mul(self.height as usize)
            .expect("notification buffer size overflow");
        let mut pool = SlotPool::new(bytes, shm).expect("allocate notification buffer pool");
        let (buffer, canvas) = pool
            .create_buffer(
                self.width as i32,
                self.height as i32,
                stride as i32,
                wl_shm::Format::Argb8888,
            )
            .expect("allocate notification buffer");
        render::card::render_card(
            canvas,
            self.width,
            self.height,
            self.full_width,
            self.full_height,
            notification,
            style,
            fonts,
        );
        fonts.clear_raster_cache();

        self.layer
            .wl_surface()
            .damage_buffer(0, 0, self.width as i32, self.height as i32);
        buffer
            .attach_to(self.layer.wl_surface())
            .expect("attach notification buffer");
        self.layer.commit();

        self.frame.replace(Frame {
            buffer,
            _pool: pool,
        })
    }

    pub(super) fn take_frame(&mut self) -> Option<Frame> {
        self.frame.take()
    }
}

fn anchor_for(anchor: &PositionAnchor) -> Anchor {
    match anchor {
        PositionAnchor::Top => Anchor::TOP,
        PositionAnchor::TopLeft => Anchor::TOP | Anchor::LEFT,
        PositionAnchor::TopRight => Anchor::TOP | Anchor::RIGHT,
        PositionAnchor::Bottom => Anchor::BOTTOM,
        PositionAnchor::BottomLeft => Anchor::BOTTOM | Anchor::LEFT,
        PositionAnchor::BottomRight => Anchor::BOTTOM | Anchor::RIGHT,
        PositionAnchor::Left => Anchor::LEFT,
        PositionAnchor::Right => Anchor::RIGHT,
    }
}

pub(super) fn restack(
    surfaces: &BTreeMap<u32, Vec<NotificationSurface>>,
    ordered_ids: &[u32],
    config: &PigeonConfig,
) {
    let position = &config.notification.position;
    let mut outputs = Vec::new();
    for surface in surfaces.values().flatten() {
        if !outputs.iter().any(|output| output == &surface.output) {
            outputs.push(surface.output.clone());
        }
    }

    for output in outputs {
        let mut offset = match position.anchor {
            PositionAnchor::Top | PositionAnchor::TopLeft | PositionAnchor::TopRight => {
                position.top_margin as i32
            }
            PositionAnchor::Bottom | PositionAnchor::BottomLeft | PositionAnchor::BottomRight => {
                position.bottom_margin as i32
            }
            PositionAnchor::Left => position.left_margin as i32,
            PositionAnchor::Right => position.right_margin as i32,
        };

        for id in ordered_ids {
            let Some(notification_surfaces) = surfaces.get(id) else {
                continue;
            };
            let Some(surface) = notification_surfaces
                .iter()
                .find(|surface| surface.output == output)
            else {
                continue;
            };

            let margins = match position.anchor {
                PositionAnchor::Top => (offset, 0, 0, 0),
                PositionAnchor::TopLeft => (offset, 0, 0, position.left_margin as i32),
                PositionAnchor::TopRight => (offset, position.right_margin as i32, 0, 0),
                PositionAnchor::Bottom => (0, 0, offset, 0),
                PositionAnchor::BottomLeft => (0, 0, offset, position.left_margin as i32),
                PositionAnchor::BottomRight => (0, position.right_margin as i32, offset, 0),
                PositionAnchor::Left => (0, 0, 0, offset),
                PositionAnchor::Right => (0, offset, 0, 0),
            };
            surface
                .layer
                .set_margin(margins.0, margins.1, margins.2, margins.3);
            surface.layer.commit();

            let size = match position.anchor {
                PositionAnchor::Left | PositionAnchor::Right => surface.width,
                _ => surface.height,
            } as i32;
            offset += size + position.notification_gap as i32;
        }
    }
}
