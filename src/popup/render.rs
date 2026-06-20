use super::Popup;
use cosmic_text::{Attrs, Buffer, Color, FontSystem, Metrics, Shaping, SwashCache, Weight, Wrap};

impl Popup {
    pub(super) fn draw_text(
        canvas: &mut [u8],
        canvas_width: u32,
        canvas_height: u32,
        font_system: &mut FontSystem,
        swash_cache: &mut SwashCache,
        text: &str,
        x_offset: i32,
        y_offset: i32,
        font_size: f32,
        bold: bool,
    ) {
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
}
