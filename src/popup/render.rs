use cosmic_text::{Attrs, Buffer, Color, FontSystem, Metrics, Shaping, SwashCache, Weight, Wrap};

use crate::{config::PigeonConfig, images::Image, notification::Notification};

pub struct FontCtx {
    pub font_system: FontSystem,
    pub swash_cache: SwashCache,
}

impl FontCtx {
    pub fn new() -> Self {
        Self {
            font_system: FontSystem::new(),
            swash_cache: SwashCache::new(),
        }
    }
}

pub fn render_card(
    canvas: &mut [u8],
    width: u32,
    height: u32,
    full_width: u32,
    full_height: u32,
    notification: &Notification,
    fonts: &mut FontCtx,
    config: &PigeonConfig,
) {
    let notification_config = &config.notification;
    fill_notification_background(
        canvas,
        width,
        height,
        full_width,
        full_height,
        notification_config.background_color,
        notification_config.border.color,
        notification_config.border.width,
        notification_config.corner_radius,
    );

    let outer_padding = notification_config
        .outer_padding
        .saturating_add(notification_config.border.width);
    let thumbnail_size = notification_config.thumbnail.size;
    let thumbnail_gap = notification_config.thumbnail.gap;
    let summary_font_size = notification_config.summary.font_size;
    let body_font_size = notification_config.body.font_size;
    let summary_body_gap = notification_config.summary.bottom_gap;

    let text_x = if let Some(img) = &notification.img {
        draw_thumbnail(
            canvas,
            width,
            height,
            img,
            outer_padding,
            outer_padding,
            thumbnail_size,
            thumbnail_size,
        );
        outer_padding + thumbnail_size + thumbnail_gap
    } else {
        outer_padding
    };
    let text_width = full_width
        .saturating_sub(text_x)
        .saturating_sub(outer_padding);
    let summary_height = measure_text_height(
        fonts,
        &notification.summary,
        text_width,
        summary_font_size,
        true,
    );
    let body_y = outer_padding as f32 + summary_height + summary_body_gap;

    let body_y = body_y.ceil() as u32;
    let body_height = full_height
        .saturating_sub(body_y)
        .saturating_sub(outer_padding);

    draw_text(
        canvas,
        width,
        height,
        fonts,
        &notification.summary,
        text_x,
        outer_padding,
        text_width,
        summary_height.ceil() as u32,
        summary_font_size,
        true,
    );

    draw_text(
        canvas,
        width,
        height,
        fonts,
        &notification.body,
        text_x,
        body_y,
        text_width,
        body_height,
        body_font_size,
        false,
    );
}

fn fill_notification_background(
    canvas: &mut [u8],
    width: u32,
    height: u32,
    full_width: u32,
    full_height: u32,
    background: [u8; 4],
    border: [u8; 4],
    border_width: u32,
    corner_radius: u32,
) {
    let border_width = border_width.min(full_width / 2).min(full_height / 2);
    let inner_width = full_width.saturating_sub(border_width.saturating_mul(2));
    let inner_height = full_height.saturating_sub(border_width.saturating_mul(2));
    let inner_radius = corner_radius.saturating_sub(border_width);

    for y in 0..height {
        for x in 0..width {
            let pixel = ((y * width + x) * 4) as usize;
            let color = match () {
                _ if !rounded_rect_contains(x, y, full_width, full_height, corner_radius) => {
                    [0, 0, 0, 0]
                }
                _ if border_width == 0 => background,
                _ if inner_width > 0
                    && inner_height > 0
                    && x >= border_width
                    && y >= border_width
                    && rounded_rect_contains(
                        x - border_width,
                        y - border_width,
                        inner_width,
                        inner_height,
                        inner_radius,
                    ) =>
                {
                    background
                }
                _ => border,
            };
            canvas[pixel..pixel + 4].copy_from_slice(&color);
        }
    }
}

fn rounded_rect_contains(x: u32, y: u32, width: u32, height: u32, corner_radius: u32) -> bool {
    if x >= width || y >= height {
        return false;
    }

    let radius = corner_radius.min(width / 2).min(height / 2);
    if radius == 0 || (x >= radius && x < width - radius) || (y >= radius && y < height - radius) {
        return true;
    }

    let center_x = if x < radius {
        radius as u64 * 2
    } else {
        (width - radius) as u64 * 2
    };
    let center_y = if y < radius {
        radius as u64 * 2
    } else {
        (height - radius) as u64 * 2
    };
    let pixel_x = x as u64 * 2 + 1;
    let pixel_y = y as u64 * 2 + 1;
    let radius = radius as u64 * 2;

    center_x.abs_diff(pixel_x).pow(2) + center_y.abs_diff(pixel_y).pow(2) <= radius.pow(2)
}

fn draw_text(
    canvas: &mut [u8],
    canvas_width: u32,
    canvas_height: u32,
    fonts: &mut FontCtx,
    text: &str,
    x_offset: u32,
    y_offset: u32,
    text_width: u32,
    text_height: u32,
    font_size: f32,
    bold: bool,
) {
    let FontCtx {
        font_system,
        swash_cache,
    } = fonts;

    let mut text_buffer = Buffer::new(font_system, Metrics::new(font_size, font_size * 1.3));
    text_buffer.set_size(Some(text_width as f32), Some(text_height as f32));
    text_buffer.set_wrap(Wrap::WordOrGlyph);

    let attrs = if bold {
        Attrs::new().weight(Weight::BOLD)
    } else {
        Attrs::new()
    };

    text_buffer.set_text(text, &attrs, Shaping::Advanced, None);

    text_buffer.draw(
        font_system,
        swash_cache,
        Color::rgb(0xFF, 0xFF, 0xFF),
        |x, y, width, height, color| {
            let x_offset = i32::try_from(x_offset).unwrap_or(i32::MAX);
            let y_offset = i32::try_from(y_offset).unwrap_or(i32::MAX);
            let glyph_width = i32::try_from(width).unwrap_or(i32::MAX);
            let glyph_height = i32::try_from(height).unwrap_or(i32::MAX);
            let max_x = i32::try_from(canvas_width).unwrap_or(i32::MAX);
            let max_y = i32::try_from(canvas_height).unwrap_or(i32::MAX);

            let x_start = x.saturating_add(x_offset).clamp(0, max_x) as u32;
            let y_start = y.saturating_add(y_offset).clamp(0, max_y) as u32;
            let x_end = x
                .saturating_add(x_offset)
                .saturating_add(glyph_width)
                .clamp(0, max_x) as u32;
            let y_end = y
                .saturating_add(y_offset)
                .saturating_add(glyph_height)
                .clamp(0, max_y) as u32;

            for y in y_start..y_end {
                for x in x_start..x_end {
                    let pixel = ((y * canvas_width + x) * 4) as usize;
                    let alpha = color.a() as u16;
                    let inverse_alpha = 255 - alpha;

                    canvas[pixel] = ((color.b() as u16 * alpha
                        + canvas[pixel] as u16 * inverse_alpha)
                        / 255) as u8;
                    canvas[pixel + 1] = ((color.g() as u16 * alpha
                        + canvas[pixel + 1] as u16 * inverse_alpha)
                        / 255) as u8;
                    canvas[pixel + 2] = ((color.r() as u16 * alpha
                        + canvas[pixel + 2] as u16 * inverse_alpha)
                        / 255) as u8;
                    canvas[pixel + 3] = 0xFF;
                }
            }
        },
    );
}

fn draw_thumbnail(
    canvas: &mut [u8],
    canvas_width: u32,
    canvas_height: u32,
    image: &Image,
    box_x: u32,
    box_y: u32,
    box_width: u32,
    box_height: u32,
) {
    let thumbnail = image.inner().thumbnail(box_width, box_height).to_rgba8();

    let draw_x = box_x + (box_width - thumbnail.width()) / 2;
    let draw_y = box_y + (box_height - thumbnail.height()) / 2;

    for (x, y, pixel) in thumbnail.enumerate_pixels() {
        let target_x = draw_x + x;
        let target_y = draw_y + y;

        if target_x >= canvas_width || target_y >= canvas_height {
            continue;
        }

        let [red, green, blue, alpha] = pixel.0;
        let index = ((target_y * canvas_width + target_x) * 4) as usize;

        let alpha = u16::from(alpha);
        let inverse_alpha = 255 - alpha;

        canvas[index] =
            ((u16::from(blue) * alpha + u16::from(canvas[index]) * inverse_alpha) / 255) as u8;

        canvas[index + 1] =
            ((u16::from(green) * alpha + u16::from(canvas[index + 1]) * inverse_alpha) / 255) as u8;

        canvas[index + 2] =
            ((u16::from(red) * alpha + u16::from(canvas[index + 2]) * inverse_alpha) / 255) as u8;

        canvas[index + 3] = 0xFF;
    }
}

pub fn measure_card_height(
    notification: &Notification,
    width: u32,
    fonts: &mut FontCtx,
    config: &PigeonConfig,
) -> u32 {
    let notification_config = &config.notification;
    let content_inset = notification_config
        .outer_padding
        .saturating_add(notification_config.border.width);
    let text_x = if notification.img.is_some() {
        content_inset + notification_config.thumbnail.size + notification_config.thumbnail.gap
    } else {
        content_inset
    };
    let text_width = width.saturating_sub(text_x).saturating_sub(content_inset);
    let summary_height = measure_text_height(
        fonts,
        &notification.summary,
        text_width,
        notification_config.summary.font_size,
        true,
    );
    let body_height = measure_text_height(
        fonts,
        &notification.body,
        text_width,
        notification_config.body.font_size,
        false,
    );
    let text_stack_height = summary_height + notification_config.summary.bottom_gap + body_height;
    let content_height = if notification.img.is_some() {
        (notification_config.thumbnail.size as f32).max(text_stack_height)
    } else {
        text_stack_height
    };

    ((content_inset.saturating_mul(2) as f32 + content_height).ceil() as u32).clamp(
        notification_config.min_height,
        notification_config.max_height,
    )
}

fn measure_text_height(
    fonts: &mut FontCtx,
    text: &str,
    width: u32,
    font_size: f32,
    bold: bool,
) -> f32 {
    let FontCtx { font_system, .. } = fonts;
    let mut text_buffer = Buffer::new(font_system, Metrics::new(font_size, font_size * 1.3));
    text_buffer.set_size(Some(width as f32), None);
    text_buffer.set_wrap(Wrap::WordOrGlyph);

    let attrs = if bold {
        Attrs::new().weight(Weight::BOLD)
    } else {
        Attrs::new()
    };
    text_buffer.set_text(text, &attrs, Shaping::Advanced, None);
    text_buffer.shape_until_scroll(font_system, false);

    text_buffer
        .layout_runs()
        .map(|run| run.line_top + run.line_height)
        .last()
        .unwrap_or(0.0)
}
