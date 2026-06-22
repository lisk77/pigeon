use cosmic_text::{
    Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, Style, SwashCache, Weight, Wrap,
};

use crate::config::notification::TextStyleConfig;

pub struct FontCtx {
    pub font_system: FontSystem,
    pub swash_cache: SwashCache,
}

pub struct StyledTextRun<'a> {
    pub text: &'a str,
    pub style: &'a TextStyleConfig,
}

impl FontCtx {
    pub fn new() -> Self {
        Self {
            font_system: FontSystem::new(),
            swash_cache: SwashCache::new(),
        }
    }

    /// Drop rasterized glyphs after a complete card render.
    ///
    /// `SwashCache` has no eviction policy, so retaining it would make memory
    /// use grow with every distinct glyph/font/size combination ever shown.
    pub fn clear_raster_cache(&mut self) {
        self.swash_cache = SwashCache::new();
    }
}

#[allow(dead_code)]
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

#[allow(dead_code)]
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

pub fn draw_styled_text(
    canvas: &mut [u8],
    canvas_width: u32,
    canvas_height: u32,
    fonts: &mut FontCtx,
    runs: &[StyledTextRun<'_>],
    x_offset: u32,
    y_offset: u32,
    text_width: u32,
    text_height: u32,
    default_style: &TextStyleConfig,
) {
    let FontCtx {
        font_system,
        swash_cache,
    } = fonts;
    let default_attrs = attrs_for(default_style);
    let mut text_buffer = Buffer::new(
        font_system,
        Metrics::new(default_style.font_size, default_style.font_size * 1.3),
    );
    text_buffer.set_size(Some(text_width as f32), Some(text_height as f32));
    text_buffer.set_wrap(Wrap::WordOrGlyph);
    text_buffer.set_rich_text(
        runs.iter().map(|run| (run.text, attrs_for(run.style))),
        &default_attrs,
        Shaping::Advanced,
        None,
    );

    draw_buffer(
        canvas,
        canvas_width,
        canvas_height,
        font_system,
        swash_cache,
        &mut text_buffer,
        x_offset,
        y_offset,
    );
}

pub fn measure_styled_text_height(
    fonts: &mut FontCtx,
    runs: &[StyledTextRun<'_>],
    width: u32,
    default_style: &TextStyleConfig,
) -> f32 {
    let FontCtx { font_system, .. } = fonts;
    let default_attrs = attrs_for(default_style);
    let mut text_buffer = Buffer::new(
        font_system,
        Metrics::new(default_style.font_size, default_style.font_size * 1.3),
    );
    text_buffer.set_size(Some(width as f32), None);
    text_buffer.set_wrap(Wrap::WordOrGlyph);
    text_buffer.set_rich_text(
        runs.iter().map(|run| (run.text, attrs_for(run.style))),
        &default_attrs,
        Shaping::Advanced,
        None,
    );
    text_buffer.shape_until_scroll(font_system, false);

    text_buffer
        .layout_runs()
        .map(|run| run.line_top + run.line_height)
        .last()
        .unwrap_or(0.0)
}

fn attrs_for(style: &TextStyleConfig) -> Attrs<'_> {
    let mut attrs = Attrs::new()
        .color(Color::rgba(
            style.color[2],
            style.color[1],
            style.color[0],
            style.color[3],
        ))
        .metrics(Metrics::new(style.font_size, style.font_size * 1.3))
        .weight(if style.bold {
            Weight::BOLD
        } else {
            Weight::NORMAL
        })
        .style(if style.italic {
            Style::Italic
        } else {
            Style::Normal
        });

    if let Some(family) = &style.font_family {
        attrs = attrs.family(Family::Name(family));
    }

    attrs
}

fn draw_buffer(
    canvas: &mut [u8],
    canvas_width: u32,
    canvas_height: u32,
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    text_buffer: &mut Buffer,
    x_offset: u32,
    y_offset: u32,
) {
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
