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

struct EmojiTextRun<'a> {
    text: &'a str,
    style: &'a TextStyleConfig,
    is_emoji: bool,
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
    emoji_font: &str,
) {
    let FontCtx {
        font_system,
        swash_cache,
    } = fonts;
    let default_attrs = attrs_for(default_style, None);
    let emoji_font = (!emoji_font.is_empty()).then_some(emoji_font);
    let runs = split_emoji_runs(runs);
    let mut text_buffer = Buffer::new(
        font_system,
        Metrics::new(default_style.font_size, default_style.font_size * 1.3),
    );
    text_buffer.set_size(Some(text_width as f32), Some(text_height as f32));
    text_buffer.set_wrap(Wrap::WordOrGlyph);
    text_buffer.set_rich_text(
        runs.iter().map(|run| {
            let family = run.is_emoji.then_some(emoji_font).flatten();
            (run.text, attrs_for(run.style, family))
        }),
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
    emoji_font: &str,
) -> f32 {
    let FontCtx { font_system, .. } = fonts;
    let default_attrs = attrs_for(default_style, None);
    let emoji_font = (!emoji_font.is_empty()).then_some(emoji_font);
    let runs = split_emoji_runs(runs);
    let mut text_buffer = Buffer::new(
        font_system,
        Metrics::new(default_style.font_size, default_style.font_size * 1.3),
    );
    text_buffer.set_size(Some(width as f32), None);
    text_buffer.set_wrap(Wrap::WordOrGlyph);
    text_buffer.set_rich_text(
        runs.iter().map(|run| {
            let family = run.is_emoji.then_some(emoji_font).flatten();
            (run.text, attrs_for(run.style, family))
        }),
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

fn attrs_for<'a>(style: &'a TextStyleConfig, family_override: Option<&'a str>) -> Attrs<'a> {
    // Color-emoji fonts commonly only provide a regular face.  In particular,
    // asking for a bold summary glyph can make font matching fall back to the
    // surrounding monochrome text font, even though the family was explicitly
    // set to Noto Color Emoji.
    let uses_override = family_override.is_some();
    let mut attrs = Attrs::new()
        .color(Color::rgba(
            style.color[2],
            style.color[1],
            style.color[0],
            style.color[3],
        ))
        .metrics(Metrics::new(style.font_size, style.font_size * 1.3))
        .weight(if style.bold && !uses_override {
            Weight::BOLD
        } else {
            Weight::NORMAL
        })
        .style(if style.italic && !uses_override {
            Style::Italic
        } else {
            Style::Normal
        });

    if let Some(family) = family_override.or(style.font_family.as_deref()) {
        attrs = attrs.family(Family::Name(family));
    }

    attrs
}

fn split_emoji_runs<'a>(runs: &'a [StyledTextRun<'a>]) -> Vec<EmojiTextRun<'a>> {
    let mut result = Vec::new();

    for run in runs {
        let mut start = 0;
        let mut is_emoji = false;

        for (index, character) in run.text.char_indices() {
            let next_is_emoji = is_emoji_character(character)
                || (is_emoji && is_emoji_sequence_character(character));
            if index != start && next_is_emoji != is_emoji {
                result.push(EmojiTextRun {
                    text: &run.text[start..index],
                    style: run.style,
                    is_emoji,
                });
                start = index;
            }
            is_emoji = next_is_emoji;
        }

        if start != run.text.len() {
            result.push(EmojiTextRun {
                text: &run.text[start..],
                style: run.style,
                is_emoji,
            });
        }
    }

    result
}

fn is_emoji_character(character: char) -> bool {
    matches!(
        character as u32,
        0x00A9
            | 0x00AE
            | 0x203C
            | 0x2049
            | 0x2122
            | 0x2139
            | 0x2194..=0x21FF
            | 0x2300..=0x23FF
            | 0x2600..=0x27BF
            | 0x2934..=0x2935
            | 0x2B05..=0x2B55
            | 0x3030
            | 0x303D
            | 0x3297
            | 0x3299
            | 0x1F000..=0x1FAFF
    )
}

fn is_emoji_sequence_character(character: char) -> bool {
    matches!(character, '\u{200D}' | '\u{20E3}' | '\u{FE0E}' | '\u{FE0F}')
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
