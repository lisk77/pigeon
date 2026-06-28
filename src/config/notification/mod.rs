use serde::{Deserialize, Deserializer, Serialize, Serializer, ser::SerializeSeq};

mod animation;
mod body;
mod border;
mod overrides;
mod position;
mod progress;
mod summary;
mod template;
mod text;
mod thumbnail;

pub use animation::{AnimationConfig, AnimationDirection, AnimationEffect, TransitionConfig};
pub use overrides::NotificationStyleOverride;
pub use position::{Anchor, PositionConfig};
pub use progress::{ProgressAlignment, ProgressConfig, ProgressDirection, ProgressThickness};
pub use template::{NotificationTemplate, TemplateElement, TemplateRun};
pub use text::TextStyleConfig;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct NotificationConfig {
    pub below_fullscreen: bool,
    pub animation: animation::AnimationConfig,
    pub gradient_direction: GradientDirection,
    pub min_width: u32,
    pub max_width: u32,
    pub min_height: u32,
    pub max_height: u32,
    pub outer_padding: u32,
    pub corner_radius: u32,
    pub format: template::NotificationTemplate,
    pub color: ColorConfig,
    pub emoji_font: String,
    pub position: position::PositionConfig,
    pub progress: progress::ProgressConfig,
    pub border: border::BorderConfig,
    pub thumbnail: thumbnail::ThumbnailConfig,
    pub summary: summary::SummaryConfig,
    pub body: body::BodyConfig,
    pub app_name: text::TextStyleConfig,
    pub details: text::TextStyleConfig,
    pub literal: text::TextStyleConfig,
}

impl Default for NotificationConfig {
    fn default() -> Self {
        Self {
            below_fullscreen: false,
            animation: animation::AnimationConfig::default(),
            gradient_direction: GradientDirection::Horizontal,
            min_width: 240,
            max_width: 360,
            min_height: 96,
            max_height: 480,
            outer_padding: 16,
            corner_radius: 12,
            format: template::NotificationTemplate::default(),
            color: ColorConfig::solid([0x20, 0x20, 0x20, 0xff]),
            emoji_font: "Noto Color Emoji".into(),
            position: position::PositionConfig::default(),
            progress: progress::ProgressConfig::default(),
            border: border::BorderConfig::default(),
            thumbnail: thumbnail::ThumbnailConfig::default(),
            summary: summary::SummaryConfig::default(),
            body: body::BodyConfig::default(),
            app_name: TextStyleConfig {
                font_size: 12.0,
                bold: true,
                ..TextStyleConfig::default()
            },
            details: TextStyleConfig {
                font_size: 12.0,
                color: ColorConfig::solid([0xa0, 0xa0, 0xa0, 0xff]),
                ..TextStyleConfig::default()
            },
            literal: TextStyleConfig::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColorConfig {
    colors: Vec<RgbaColor>,
}

pub type RgbaColor = [u8; 4];

impl ColorConfig {
    pub fn solid(color: RgbaColor) -> Self {
        Self {
            colors: vec![color],
        }
    }

    pub fn first(&self) -> RgbaColor {
        self.colors[0]
    }

    pub fn at(
        &self,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        direction: GradientDirection,
    ) -> RgbaColor {
        if self.colors.len() == 1 {
            return self.colors[0];
        }

        let (position, max_position) = match direction {
            GradientDirection::Horizontal => (x, width.saturating_sub(1)),
            GradientDirection::Vertical => (y, height.saturating_sub(1)),
            GradientDirection::Diagonal => (
                x.saturating_add(y),
                width
                    .saturating_sub(1)
                    .saturating_add(height.saturating_sub(1)),
            ),
            GradientDirection::DiagonalReverse => (
                width.saturating_sub(1).saturating_sub(x).saturating_add(y),
                width
                    .saturating_sub(1)
                    .saturating_add(height.saturating_sub(1)),
            ),
        };
        if max_position == 0 {
            return self.colors[0];
        }

        let stop_count = self.colors.len() - 1;
        let scaled = u64::from(position.min(max_position)) * stop_count as u64;
        let max_position = u64::from(max_position);
        let index = (scaled / max_position) as usize;
        if index >= stop_count {
            return self.colors[stop_count];
        }

        let remainder = scaled % max_position;
        interpolate_color(
            self.colors[index],
            self.colors[index + 1],
            remainder,
            max_position,
        )
    }
}

impl<'de> Deserialize<'de> for ColorConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum RawColorConfig {
            Solid(String),
            Gradient(Vec<String>),
        }

        let colors = match RawColorConfig::deserialize(deserializer)? {
            RawColorConfig::Solid(color) => {
                vec![parse_rgba_color(&color).map_err(serde::de::Error::custom)?]
            }
            RawColorConfig::Gradient(colors) => {
                if colors.is_empty() {
                    return Err(serde::de::Error::custom(
                        "color array must contain at least one color",
                    ));
                }
                colors
                    .into_iter()
                    .map(|color| parse_rgba_color(&color).map_err(serde::de::Error::custom))
                    .collect::<Result<Vec<_>, _>>()?
            }
        };

        Ok(Self { colors })
    }
}

impl Serialize for ColorConfig {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if self.colors.len() == 1 {
            return serializer.serialize_str(&format_rgba_color(self.colors[0]));
        }

        let mut sequence = serializer.serialize_seq(Some(self.colors.len()))?;
        for color in &self.colors {
            sequence.serialize_element(&format_rgba_color(*color))?;
        }
        sequence.end()
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GradientDirection {
    #[default]
    Horizontal,
    Vertical,
    Diagonal,
    DiagonalReverse,
}

pub(super) fn deserialize_rgba_color<'de, D>(deserializer: D) -> Result<ColorConfig, D::Error>
where
    D: Deserializer<'de>,
{
    ColorConfig::deserialize(deserializer)
}

pub(super) fn serialize_rgba_color<S>(color: &ColorConfig, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    color.serialize(serializer)
}

fn format_rgba_color(color: RgbaColor) -> String {
    let [blue, green, red, alpha] = color;
    let value = if alpha == 0xff {
        format!("#{red:02x}{green:02x}{blue:02x}")
    } else {
        format!("#{red:02x}{green:02x}{blue:02x}{alpha:02x}")
    };
    value
}

pub(super) fn parse_rgba_color(value: &str) -> Result<RgbaColor, String> {
    let hex = value.strip_prefix('#').unwrap_or(value);

    if !hex.is_ascii() || !matches!(hex.len(), 6 | 8) {
        return Err("color must be #RRGGBB or #RRGGBBAA".into());
    }

    let parse_component = |offset| {
        u8::from_str_radix(&hex[offset..offset + 2], 16)
            .map_err(|_| "color must contain only hexadecimal digits".to_string())
    };

    let alpha = if hex.len() == 8 {
        parse_component(6)?
    } else {
        0xff
    };

    let red = parse_component(0)?;
    let green = parse_component(2)?;
    let blue = parse_component(4)?;

    Ok([blue, green, red, alpha])
}

fn interpolate_color(
    from: RgbaColor,
    to: RgbaColor,
    numerator: u64,
    denominator: u64,
) -> RgbaColor {
    let interpolate = |from: u8, to: u8| {
        ((u64::from(from) * (denominator - numerator)
            + u64::from(to) * numerator
            + denominator / 2)
            / denominator) as u8
    };

    [
        interpolate(from[0], to[0]),
        interpolate(from[1], to[1]),
        interpolate(from[2], to[2]),
        interpolate(from[3], to[3]),
    ]
}
