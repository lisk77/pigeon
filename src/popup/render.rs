use cosmic_text::{Attrs, Buffer, Color, FontSystem, Metrics, Shaping, SwashCache, Weight, Wrap};

use crate::notification::Notification;

pub const CARD_WIDTH: u32 = 360;
pub const CARD_HEIGHT: u32 = 160;

const PADDING_X: i32 = 16;
const SUMMARY_Y: i32 = 18;
const BODY_Y: i32 = 48;
const SUMMARY_SIZE: f32 = 18.0;
const BODY_SIZE: f32 = 14.0;
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

pub fn render_card(canvas: &mut [u8], width: u32, height: u32, notification: &Notification, fonts: &mut FontCtx) {
    for pixel in canvas.chunks_exact_mut(4) {
        pixel.copy_from_slice(&BACKGROUND);
    }

    draw_text(
        canvas,
        width,
        height,
        fonts,
        &notification.summary,
        PADDING_X,
        SUMMARY_Y,
        SUMMARY_SIZE,
        true,
    );

    draw_text(
        canvas,
        width,
        height,
        fonts,
        &notification.body,
        PADDING_X,
        BODY_Y,
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
    font_size: f32,
    bold: bool,
) {
    let FontCtx { font_system, swash_cache } = fonts;

    let mut text_buffer = Buffer::new(font_system, Metrics::new(font_size, font_size * 1.3));
    text_buffer.set_size(
        Some((canvas_width as i32 - x_offset * 2) as f32),
        Some((canvas_height as i32 - y_offset) as f32),
    );
    text_buffer.set_wrap(Wrap::Word);

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
