use cosmic_text::{Attrs, Buffer, Color, FontSystem, Metrics, Shaping, SwashCache, Weight, Wrap};

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

pub fn draw_text(
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

pub fn measure_text_height(
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
