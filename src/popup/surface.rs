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
        notification::{Anchor as PositionAnchor, AnimationEffect, PositionConfig},
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
    base_pixels: Option<Vec<u8>>,
    margins: Margins,
    transition_edge: AnimatedEdge,
    transition: Option<Transition>,
    frame_pending: bool,
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
        below_fullscreen: bool,
        position: &PositionConfig,
        transition_edge: AnimatedEdge,
    ) -> Self {
        let wl_surface = compositor.create_surface(qh);
        let layer = layer_shell.create_layer_surface(
            qh,
            wl_surface,
            layer_for(below_fullscreen),
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
            base_pixels: None,
            margins: Margins::default(),
            transition_edge,
            transition: None,
            frame_pending: false,
        }
    }

    pub(super) fn update_position(&mut self, position: &PositionConfig) {
        self.layer.set_anchor(anchor_for(&position.anchor));
        self.apply_margins();
    }

    pub(super) fn update_transition_edge(&mut self, edge: AnimatedEdge) {
        self.transition_edge = edge;
        self.apply_margins();
    }

    pub(super) fn update_below_fullscreen(&self, below_fullscreen: bool) {
        self.layer.set_layer(layer_for(below_fullscreen));
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
        self.base_pixels = visual_effects_enabled(style).then(|| canvas.to_vec());
        self.apply_visual_transition_to(canvas);
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

    fn draw_transition_frame(&mut self, shm: &Shm) -> Option<Frame> {
        let transition = self.transition?;
        transition.visual_progress()?;
        self.base_pixels.as_ref()?;
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
        self.apply_visual_transition_to(canvas);

        self.layer
            .wl_surface()
            .damage_buffer(0, 0, self.width as i32, self.height as i32);
        buffer
            .attach_to(self.layer.wl_surface())
            .expect("attach notification buffer");

        self.frame.replace(Frame {
            buffer,
            _pool: pool,
        })
    }

    fn apply_visual_transition_to(&self, canvas: &mut [u8]) {
        let Some(transition) = self.transition else {
            return;
        };
        let Some(progress) = transition.visual_progress() else {
            return;
        };
        let Some(base_pixels) = &self.base_pixels else {
            return;
        };

        match transition.effect {
            AnimationEffect::Fade | AnimationEffect::SlideFade => {
                copy_with_opacity(base_pixels, canvas, progress);
            }
            AnimationEffect::Scale => {
                copy_scaled(
                    base_pixels,
                    canvas,
                    self.width,
                    self.height,
                    scale_factor(progress),
                );
            }
            AnimationEffect::None | AnimationEffect::Slide => {}
        }
    }

    pub(super) fn start_enter(
        &mut self,
        duration: u32,
        edge: AnimatedEdge,
        effect: AnimationEffect,
    ) {
        if duration == 0 {
            return;
        }
        self.transition_edge = edge;
        self.transition = Some(Transition::new(TransitionPhase::Enter, duration, effect));
    }

    pub(super) fn start_exit(
        &mut self,
        duration: u32,
        edge: AnimatedEdge,
        effect: AnimationEffect,
    ) {
        if duration == 0 {
            return;
        }
        self.transition_edge = edge;
        self.transition = Some(Transition::new(TransitionPhase::Exit, duration, effect));
        self.frame_pending = false;
    }

    pub(super) fn transitioning(&self) -> bool {
        self.transition.is_some()
    }

    pub(super) fn request_transition_frame(&mut self, qh: &QueueHandle<Popup>) {
        if self.transition.is_none() || self.frame_pending {
            return;
        }

        let surface = self.layer.wl_surface();
        surface.frame(qh, surface.clone());
        self.frame_pending = true;
        self.layer.commit();
    }

    pub(super) fn transition_frame(&mut self, time: u32, shm: &Shm) -> (bool, Option<Frame>) {
        self.frame_pending = false;

        let Some(transition) = &mut self.transition else {
            return (false, None);
        };
        let phase = transition.phase;
        let complete = transition.update(time);
        let retired_frame = self.draw_transition_frame(shm);
        self.apply_margins();
        self.layer.commit();

        if complete {
            self.transition = None;
            if matches!(phase, TransitionPhase::Move { .. }) {
                self.apply_margins();
                self.layer.commit();
            }
            return (true, retired_frame);
        }

        (false, retired_frame)
    }

    pub(super) fn set_margins(&mut self, margins: Margins, transition_duration: Option<u32>) {
        let displayed_margins = self.displayed_margins();
        let duration = transition_duration.filter(|duration| *duration > 0);
        let transition_can_move = self.transition.is_none()
            || self
                .transition
                .is_some_and(|transition| matches!(transition.phase, TransitionPhase::Move { .. }));
        let should_transition = self.configured
            && transition_can_move
            && displayed_margins != margins
            && duration.is_some();
        self.margins = margins;
        if should_transition {
            self.transition = Some(Transition::new(
                TransitionPhase::Move {
                    from: displayed_margins,
                    to: margins,
                },
                duration.expect("transition duration checked above"),
                AnimationEffect::None,
            ));
        }
        self.apply_margins();
    }

    pub(super) fn take_frame(&mut self) -> Option<Frame> {
        self.frame.take()
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct Margins {
    pub(super) top: i32,
    pub(super) right: i32,
    pub(super) bottom: i32,
    pub(super) left: i32,
}

#[derive(Clone, Copy)]
struct Transition {
    phase: TransitionPhase,
    effect: AnimationEffect,
    duration: u32,
    started_at: Option<u32>,
    progress: f32,
}

#[derive(Clone, Copy)]
enum TransitionPhase {
    Enter,
    Exit,
    Move { from: Margins, to: Margins },
}

impl Transition {
    fn new(phase: TransitionPhase, duration: u32, effect: AnimationEffect) -> Self {
        Self {
            phase,
            effect,
            duration,
            started_at: None,
            progress: 0.0,
        }
    }

    fn update(&mut self, time: u32) -> bool {
        let started_at = *self.started_at.get_or_insert(time);
        let elapsed = time.saturating_sub(started_at);
        self.progress = (elapsed as f32 / self.duration as f32).clamp(0.0, 1.0);
        elapsed >= self.duration
    }

    fn offset_progress(&self) -> f32 {
        if !matches!(
            self.effect,
            AnimationEffect::Slide | AnimationEffect::SlideFade
        ) {
            return 0.0;
        }

        match self.phase {
            TransitionPhase::Enter => ease_out_cubic(self.progress) - 1.0,
            TransitionPhase::Exit => -ease_in_cubic(self.progress),
            TransitionPhase::Move { .. } => 0.0,
        }
    }

    fn visual_progress(&self) -> Option<f32> {
        let progress = match self.phase {
            TransitionPhase::Enter => ease_out_cubic(self.progress),
            TransitionPhase::Exit => 1.0 - ease_in_cubic(self.progress),
            TransitionPhase::Move { .. } => return None,
        };

        match self.effect {
            AnimationEffect::Fade | AnimationEffect::SlideFade | AnimationEffect::Scale => {
                Some(progress)
            }
            AnimationEffect::None | AnimationEffect::Slide => None,
        }
    }
}

impl NotificationSurface {
    fn apply_margins(&self) {
        let margins = self.displayed_margins();
        self.layer
            .set_margin(margins.top, margins.right, margins.bottom, margins.left);
    }

    fn displayed_margins(&self) -> Margins {
        let mut margins = self.margins;
        if let Some(transition) = self.transition {
            if let TransitionPhase::Move { from, to } = transition.phase {
                return from.lerp(to, ease_out_cubic(transition.progress));
            }

            let vertical_distance = self.height.max(self.full_height) as f32;
            let horizontal_distance = self.width.max(self.full_width) as f32;
            let vertical_offset = (vertical_distance * transition.offset_progress()).round() as i32;
            let horizontal_offset =
                (horizontal_distance * transition.offset_progress()).round() as i32;

            match self.transition_edge {
                AnimatedEdge::Top => margins.top += vertical_offset,
                AnimatedEdge::Bottom => margins.bottom += vertical_offset,
                AnimatedEdge::Left => margins.left += horizontal_offset,
                AnimatedEdge::Right => margins.right += horizontal_offset,
            }
        }

        margins
    }
}

impl Margins {
    fn lerp(self, to: Self, progress: f32) -> Self {
        Self {
            top: lerp_i32(self.top, to.top, progress),
            right: lerp_i32(self.right, to.right, progress),
            bottom: lerp_i32(self.bottom, to.bottom, progress),
            left: lerp_i32(self.left, to.left, progress),
        }
    }
}

fn lerp_i32(from: i32, to: i32, progress: f32) -> i32 {
    (from as f32 + (to - from) as f32 * progress).round() as i32
}

#[derive(Clone, Copy)]
pub(super) enum AnimatedEdge {
    Top,
    Right,
    Bottom,
    Left,
}

fn ease_out_cubic(progress: f32) -> f32 {
    1.0 - (1.0 - progress).powi(3)
}

fn ease_in_cubic(progress: f32) -> f32 {
    progress.powi(3)
}

fn copy_with_opacity(source: &[u8], target: &mut [u8], opacity: f32) {
    let opacity = opacity.clamp(0.0, 1.0);
    if opacity >= 0.999 {
        target.copy_from_slice(source);
        return;
    }
    if opacity <= 0.001 {
        target.fill(0);
        return;
    }

    for (source, target) in source.iter().zip(target.iter_mut()) {
        *target = ((*source as f32) * opacity).round() as u8;
    }
}

fn copy_scaled(source: &[u8], target: &mut [u8], width: u32, height: u32, scale: f32) {
    let scale = scale.clamp(0.01, 1.0);
    if scale >= 0.999 {
        target.copy_from_slice(source);
        return;
    }

    target.fill(0);
    let scaled_width = ((width as f32) * scale).round().clamp(1.0, width as f32) as u32;
    let scaled_height = ((height as f32) * scale).round().clamp(1.0, height as f32) as u32;
    let x_offset = (width - scaled_width) / 2;
    let y_offset = (height - scaled_height) / 2;

    for target_y in 0..scaled_height {
        let source_y = ((target_y as f32) / scale).floor() as u32;
        let source_y = source_y.min(height.saturating_sub(1));
        for target_x in 0..scaled_width {
            let source_x = ((target_x as f32) / scale).floor() as u32;
            let source_x = source_x.min(width.saturating_sub(1));
            let source_index = ((source_y * width + source_x) * 4) as usize;
            let target_index = (((target_y + y_offset) * width + target_x + x_offset) * 4) as usize;
            target[target_index..target_index + 4]
                .copy_from_slice(&source[source_index..source_index + 4]);
        }
    }
}

fn scale_factor(progress: f32) -> f32 {
    0.88 + progress.clamp(0.0, 1.0) * 0.12
}

fn visual_effects_enabled(style: &NotificationConfig) -> bool {
    effect_needs_pixels(style.animation.enter.effect)
        || effect_needs_pixels(style.animation.exit.effect)
}

fn effect_needs_pixels(effect: AnimationEffect) -> bool {
    matches!(
        effect,
        AnimationEffect::Fade | AnimationEffect::SlideFade | AnimationEffect::Scale
    )
}

fn layer_for(below_fullscreen: bool) -> Layer {
    if below_fullscreen {
        Layer::Top
    } else {
        Layer::Overlay
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

pub(super) fn edge_for(anchor: &PositionAnchor) -> AnimatedEdge {
    match anchor {
        PositionAnchor::Top | PositionAnchor::TopLeft | PositionAnchor::TopRight => {
            AnimatedEdge::Top
        }
        PositionAnchor::Bottom | PositionAnchor::BottomLeft | PositionAnchor::BottomRight => {
            AnimatedEdge::Bottom
        }
        PositionAnchor::Left => AnimatedEdge::Left,
        PositionAnchor::Right => AnimatedEdge::Right,
    }
}

pub(super) fn restack(
    qh: &QueueHandle<Popup>,
    surfaces: &mut BTreeMap<u32, Vec<NotificationSurface>>,
    exiting_surfaces: &[NotificationSurface],
    ordered_ids: &[u32],
    config: &PigeonConfig,
    transition_duration: Option<u32>,
) {
    let position = &config.notification.position;
    let mut outputs = Vec::new();
    for surface in surfaces.values().flatten() {
        if !outputs.iter().any(|output| output == &surface.output) {
            outputs.push(surface.output.clone());
        }
    }
    for surface in exiting_surfaces {
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
        let mut reserved = exiting_surfaces
            .iter()
            .filter(|surface| surface.output == output)
            .map(|surface| {
                (
                    surface.stack_offset(position.anchor),
                    surface.stack_size(position.anchor),
                )
            })
            .collect::<Vec<_>>();
        reserved.sort_by_key(|(offset, _)| *offset);

        for id in ordered_ids {
            let Some(notification_surfaces) = surfaces.get_mut(id) else {
                continue;
            };
            let Some(surface) = notification_surfaces
                .iter_mut()
                .find(|surface| surface.output == output)
            else {
                continue;
            };

            offset = skip_reserved_offset(
                offset,
                surface.stack_size(position.anchor),
                position.notification_gap as i32,
                &reserved,
            );
            let margins = match position.anchor {
                PositionAnchor::Top => Margins {
                    top: offset,
                    ..Margins::default()
                },
                PositionAnchor::TopLeft => Margins {
                    top: offset,
                    left: position.left_margin as i32,
                    ..Margins::default()
                },
                PositionAnchor::TopRight => Margins {
                    top: offset,
                    right: position.right_margin as i32,
                    ..Margins::default()
                },
                PositionAnchor::Bottom => Margins {
                    bottom: offset,
                    ..Margins::default()
                },
                PositionAnchor::BottomLeft => Margins {
                    bottom: offset,
                    left: position.left_margin as i32,
                    ..Margins::default()
                },
                PositionAnchor::BottomRight => Margins {
                    right: position.right_margin as i32,
                    bottom: offset,
                    ..Margins::default()
                },
                PositionAnchor::Left => Margins {
                    left: offset,
                    ..Margins::default()
                },
                PositionAnchor::Right => Margins {
                    right: offset,
                    ..Margins::default()
                },
            };
            surface.set_margins(margins, transition_duration);
            surface.request_transition_frame(qh);
            surface.layer.commit();

            offset += surface.stack_size(position.anchor) + position.notification_gap as i32;
        }
    }
}

impl NotificationSurface {
    fn stack_offset(&self, anchor: PositionAnchor) -> i32 {
        match anchor {
            PositionAnchor::Top | PositionAnchor::TopLeft | PositionAnchor::TopRight => {
                self.margins.top
            }
            PositionAnchor::Bottom | PositionAnchor::BottomLeft | PositionAnchor::BottomRight => {
                self.margins.bottom
            }
            PositionAnchor::Left => self.margins.left,
            PositionAnchor::Right => self.margins.right,
        }
    }

    fn stack_size(&self, anchor: PositionAnchor) -> i32 {
        (match anchor {
            PositionAnchor::Left | PositionAnchor::Right => self.width,
            _ => self.height,
        }) as i32
    }
}

fn skip_reserved_offset(mut offset: i32, size: i32, gap: i32, reserved: &[(i32, i32)]) -> i32 {
    for &(reserved_offset, reserved_size) in reserved {
        if ranges_overlap(offset, size, reserved_offset, reserved_size) {
            offset = reserved_offset + reserved_size + gap;
        }
    }
    offset
}

fn ranges_overlap(offset: i32, size: i32, reserved_offset: i32, reserved_size: i32) -> bool {
    let end = offset + size;
    let reserved_end = reserved_offset + reserved_size;
    offset < reserved_end && reserved_offset < end
}
