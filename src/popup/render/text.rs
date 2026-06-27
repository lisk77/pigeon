use cosmic_text::{
    Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, Style, SwashCache, Weight, Wrap,
};

use crate::config::notification::{ColorConfig, GradientDirection, RgbaColor, TextStyleConfig};

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
    default_gradient_direction: GradientDirection,
) {
    let FontCtx {
        font_system,
        swash_cache,
    } = fonts;
    let default_attrs = attrs_for(default_style, None, 0);
    let emoji_font = (!emoji_font.is_empty()).then_some(emoji_font);
    let runs = split_emoji_runs(runs);
    let mut text_buffer = Buffer::new(
        font_system,
        Metrics::new(default_style.font_size, default_style.font_size * 1.3),
    );
    text_buffer.set_size(Some(text_width as f32), Some(text_height as f32));
    text_buffer.set_wrap(Wrap::WordOrGlyph);
    text_buffer.set_rich_text(
        runs.iter().enumerate().map(|(index, run)| {
            let family = run.is_emoji.then_some(emoji_font).flatten();
            (run.text, attrs_for(run.style, family, index + 1))
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
        &runs,
        default_gradient_direction,
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
    let default_attrs = attrs_for(default_style, None, 0);
    let emoji_font = (!emoji_font.is_empty()).then_some(emoji_font);
    let runs = split_emoji_runs(runs);
    let mut text_buffer = Buffer::new(
        font_system,
        Metrics::new(default_style.font_size, default_style.font_size * 1.3),
    );
    text_buffer.set_size(Some(width as f32), None);
    text_buffer.set_wrap(Wrap::WordOrGlyph);
    text_buffer.set_rich_text(
        runs.iter().enumerate().map(|(index, run)| {
            let family = run.is_emoji.then_some(emoji_font).flatten();
            (run.text, attrs_for(run.style, family, index + 1))
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

fn attrs_for<'a>(
    style: &'a TextStyleConfig,
    family_override: Option<&'a str>,
    metadata: usize,
) -> Attrs<'a> {
    let uses_override = family_override.is_some();
    let mut attrs = Attrs::new()
        .color({
            let color = style.color.first();
            Color::rgba(color[2], color[1], color[0], color[3])
        })
        .metadata(metadata)
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
    runs: &[EmojiTextRun<'_>],
    default_gradient_direction: GradientDirection,
) {
    let mut glyphs = Vec::new();
    text_buffer.shape_until_scroll(font_system, false);
    for run in text_buffer.layout_runs() {
        for glyph in run.glyphs {
            let physical = glyph.physical((0.0, run.line_y), 1.0);
            let color = style_color(glyph.metadata, runs)
                .map(color_for_config)
                .or(glyph.color_opt)
                .unwrap_or_else(|| Color::rgb(0xff, 0xff, 0xff));
            glyphs.push((physical, glyph.metadata, color));
        }
    }

    let x_offset = i32::try_from(x_offset).unwrap_or(i32::MAX);
    let y_offset = i32::try_from(y_offset).unwrap_or(i32::MAX);
    let max_x = i32::try_from(canvas_width).unwrap_or(i32::MAX);
    let max_y = i32::try_from(canvas_height).unwrap_or(i32::MAX);
    let mut pixels = Vec::new();
    let mut bounds: Option<TextBounds> = None;
    let mut fallback_bounds: Option<TextBounds> = None;

    for (glyph, metadata, base_color) in glyphs {
        swash_cache.with_pixels(font_system, glyph.cache_key, base_color, |x, y, color| {
            let local_x = glyph.x.saturating_add(x);
            let local_y = glyph.y.saturating_add(y);
            let Some(target_x) = local_x.checked_add(x_offset) else {
                return;
            };
            let Some(target_y) = local_y.checked_add(y_offset) else {
                return;
            };
            if target_x < 0 || target_y < 0 || target_x >= max_x || target_y >= max_y {
                return;
            }

            let local_x = local_x.max(0) as u32;
            let local_y = local_y.max(0) as u32;
            match &mut fallback_bounds {
                Some(bounds) => bounds.include(local_x, local_y),
                None => fallback_bounds = Some(TextBounds::new(local_x, local_y)),
            }
            if is_style_mask_pixel(color, style_color(metadata, runs)) {
                match &mut bounds {
                    Some(bounds) => bounds.include(local_x, local_y),
                    None => bounds = Some(TextBounds::new(local_x, local_y)),
                }
            }
            pixels.push(TextPixel {
                target_x: target_x as u32,
                target_y: target_y as u32,
                local_x,
                local_y,
                metadata,
                color,
            });
        });
    }

    let Some(bounds) = bounds.or(fallback_bounds) else {
        return;
    };

    for text_pixel in pixels {
        let color = text_pixel_color(
            text_pixel.color,
            style_for(text_pixel.metadata, runs),
            text_pixel.local_x.saturating_sub(bounds.min_x),
            text_pixel.local_y.saturating_sub(bounds.min_y),
            bounds.width(),
            bounds.height(),
            default_gradient_direction,
        );
        let pixel = ((text_pixel.target_y * canvas_width + text_pixel.target_x) * 4) as usize;
        let alpha = u16::from(color[3]);
        let inverse_alpha = 255 - alpha;

        canvas[pixel] =
            ((u16::from(color[0]) * alpha + canvas[pixel] as u16 * inverse_alpha) / 255) as u8;
        canvas[pixel + 1] =
            ((u16::from(color[1]) * alpha + canvas[pixel + 1] as u16 * inverse_alpha) / 255) as u8;
        canvas[pixel + 2] =
            ((u16::from(color[2]) * alpha + canvas[pixel + 2] as u16 * inverse_alpha) / 255) as u8;
        canvas[pixel + 3] = 0xFF;
    }
}

struct TextPixel {
    target_x: u32,
    target_y: u32,
    local_x: u32,
    local_y: u32,
    metadata: usize,
    color: Color,
}

struct TextBounds {
    min_x: u32,
    min_y: u32,
    max_x: u32,
    max_y: u32,
}

impl TextBounds {
    fn new(x: u32, y: u32) -> Self {
        Self {
            min_x: x,
            min_y: y,
            max_x: x,
            max_y: y,
        }
    }

    fn include(&mut self, x: u32, y: u32) {
        self.min_x = self.min_x.min(x);
        self.min_y = self.min_y.min(y);
        self.max_x = self.max_x.max(x);
        self.max_y = self.max_y.max(y);
    }

    fn width(&self) -> u32 {
        self.max_x.saturating_sub(self.min_x).saturating_add(1)
    }

    fn height(&self) -> u32 {
        self.max_y.saturating_sub(self.min_y).saturating_add(1)
    }
}

fn text_pixel_color(
    color: Color,
    style: Option<&TextStyleConfig>,
    x: u32,
    y: u32,
    gradient_width: u32,
    gradient_height: u32,
    default_gradient_direction: GradientDirection,
) -> RgbaColor {
    let Some(style) = style else {
        return [color.b(), color.g(), color.r(), color.a()];
    };
    let config = &style.color;
    let first = config.first();
    if color.r() != first[2] || color.g() != first[1] || color.b() != first[0] {
        return [color.b(), color.g(), color.r(), color.a()];
    }

    let sampled = config.at(
        x,
        y,
        gradient_width,
        gradient_height,
        style
            .gradient_direction
            .unwrap_or(default_gradient_direction),
    );
    let alpha = ((u32::from(color.a()) * u32::from(sampled[3])) / 255).min(255) as u8;

    [sampled[0], sampled[1], sampled[2], alpha]
}

fn is_style_mask_pixel(color: Color, config: Option<&ColorConfig>) -> bool {
    let Some(config) = config else {
        return false;
    };
    let first = config.first();
    color.r() == first[2] && color.g() == first[1] && color.b() == first[0]
}

fn style_for<'a>(metadata: usize, runs: &'a [EmojiTextRun<'_>]) -> Option<&'a TextStyleConfig> {
    metadata
        .checked_sub(1)
        .and_then(|index| runs.get(index))
        .map(|run| run.style)
}

fn style_color<'a>(metadata: usize, runs: &'a [EmojiTextRun<'_>]) -> Option<&'a ColorConfig> {
    style_for(metadata, runs).map(|style| &style.color)
}

fn color_for_config(config: &ColorConfig) -> Color {
    let color = config.first();
    Color::rgba(color[2], color[1], color[0], color[3])
}
