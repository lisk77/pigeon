use cosmic_text::{Attrs, Buffer, Color, FontSystem, Metrics, Shaping, SwashCache, Weight, Wrap};

use crate::{images::Image, notification::Notification};

pub const CARD_WIDTH: u32 = 360;
pub const CARD_HEIGHT: u32 = 160;

const PADDING: i32 = 16;
const THUMBNAIL_SIZE: u32 = 64;
const THUMBNAIL_GAP: i32 = 16;
const SUMMARY_Y: i32 = PADDING;
const SUMMARY_SIZE: f32 = 18.0;
const BODY_SIZE: f32 = 14.0;
const TEXT_GAP: f32 = 8.0;
const MIN_CARD_HEIGHT: u32 = 96;
const MAX_CARD_HEIGHT: u32 = 480;
const BACKGROUND: [u8; 4] = [0x20, 0x20, 0x20, 0xff];

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
    notification: &Notification,
    fonts: &mut FontCtx,
) {
    for pixel in canvas.chunks_exact_mut(4) {
        pixel.copy_from_slice(&BACKGROUND);
    }

    let text_x = if let Some(img) = &notification.img {
        draw_thumbnail(
            canvas,
            width,
            height,
            img,
            PADDING as u32,
            PADDING as u32,
            THUMBNAIL_SIZE,
            THUMBNAIL_SIZE,
        );
        PADDING + THUMBNAIL_SIZE as i32 + THUMBNAIL_GAP
    } else {
        PADDING
    };
    let text_width = (width as i32 - text_x - PADDING).max(0);
    let summary_height =
        measure_text_height(fonts, &notification.summary, text_width, SUMMARY_SIZE, true);
    let body_y = SUMMARY_Y as f32 + summary_height + TEXT_GAP;

    draw_text(
        canvas,
        width,
        height,
        fonts,
        &notification.summary,
        text_x,
        SUMMARY_Y,
        text_width,
        summary_height.ceil() as i32,
        SUMMARY_SIZE,
        true,
    );

    draw_text(
        canvas,
        width,
        height,
        fonts,
        &notification.body,
        text_x,
        body_y.ceil() as i32,
        text_width,
        (height as i32 - body_y.ceil() as i32 - PADDING).max(0),
        BODY_SIZE,
        false,
    );
}

fn draw_text(
    canvas: &mut [u8],
    canvas_width: u32,
    canvas_height: u32,
    fonts: &mut FontCtx,
    text: &str,
    x_offset: i32,
    y_offset: i32,
    text_width: i32,
    text_height: i32,
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
            let x_start = (x + x_offset).max(0) as u32;
            let y_start = (y + y_offset).max(0) as u32;
            let x_end = (x + x_offset + width as i32)
                .min(canvas_width as i32)
                .max(0) as u32;
            let y_end = (y + y_offset + height as i32)
                .min(canvas_height as i32)
                .max(0) as u32;

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

pub fn measure_card_height(notification: &Notification, width: u32, fonts: &mut FontCtx) -> u32 {
    let text_x = if notification.img.is_some() {
        PADDING + THUMBNAIL_SIZE as i32 + THUMBNAIL_GAP
    } else {
        PADDING
    };
    let text_width = (width as i32 - text_x - PADDING).max(0);
    let summary_height =
        measure_text_height(fonts, &notification.summary, text_width, SUMMARY_SIZE, true);
    let body_height = measure_text_height(fonts, &notification.body, text_width, BODY_SIZE, false);
    let text_stack_height = summary_height + TEXT_GAP + body_height;
    let content_height = if notification.img.is_some() {
        (THUMBNAIL_SIZE as f32).max(text_stack_height)
    } else {
        text_stack_height
    };

    (((PADDING * 2) as f32 + content_height).ceil() as u32).clamp(MIN_CARD_HEIGHT, MAX_CARD_HEIGHT)
}

fn measure_text_height(
    fonts: &mut FontCtx,
    text: &str,
    width: i32,
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
