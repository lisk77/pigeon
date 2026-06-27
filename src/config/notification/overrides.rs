use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::{
    ColorConfig, GradientDirection, NotificationConfig, NotificationTemplate, ProgressAlignment,
    ProgressDirection, ProgressThickness, TextStyleConfig, deserialize_rgba_color,
    serialize_rgba_color,
};

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct NotificationStyleOverride {
    pub outer_padding: Option<u32>,
    pub corner_radius: Option<u32>,
    pub format: Option<NotificationTemplate>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_optional_rgba_color",
        serialize_with = "serialize_optional_rgba_color"
    )]
    pub color: Option<ColorConfig>,
    pub emoji_font: Option<String>,
    pub border: BorderOverride,
    pub thumbnail: ThumbnailOverride,
    pub summary: SummaryOverride,
    pub body: BodyOverride,
    pub app_name: TextStyleOverride,
    pub details: TextStyleOverride,
    pub literal: TextStyleOverride,
    pub progress: ProgressOverride,
}

impl NotificationStyleOverride {
    pub(crate) fn is_empty(&self) -> bool {
        self.outer_padding.is_none()
            && self.corner_radius.is_none()
            && self.format.is_none()
            && self.color.is_none()
            && self.emoji_font.is_none()
            && self.border.is_empty()
            && self.thumbnail.is_empty()
            && self.summary.is_empty()
            && self.body.is_empty()
            && self.app_name.is_empty()
            && self.details.is_empty()
            && self.literal.is_empty()
            && self.progress.is_empty()
    }

    pub fn apply_to(&self, base: &NotificationConfig) -> NotificationConfig {
        let mut resolved = base.clone();

        if let Some(value) = self.outer_padding {
            resolved.outer_padding = value;
        }
        if let Some(value) = self.corner_radius {
            resolved.corner_radius = value;
        }
        if let Some(value) = &self.format {
            resolved.format = value.clone();
        }
        if let Some(value) = &self.color {
            resolved.color = value.clone();
        }
        if let Some(value) = &self.emoji_font {
            resolved.emoji_font = value.clone();
        }

        self.border.apply_to(&mut resolved);
        self.thumbnail.apply_to(&mut resolved);
        self.summary.apply_to(&mut resolved);
        self.body.apply_to(&mut resolved.body.text);
        self.app_name.apply_to(&mut resolved.app_name);
        self.details.apply_to(&mut resolved.details);
        self.literal.apply_to(&mut resolved.literal);
        self.progress.apply_to(&mut resolved);

        resolved
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct BorderOverride {
    pub width: Option<u32>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_optional_rgba_color",
        serialize_with = "serialize_optional_rgba_color"
    )]
    pub color: Option<ColorConfig>,
    pub gradient_direction: Option<GradientDirection>,
}

impl BorderOverride {
    fn is_empty(&self) -> bool {
        self.width.is_none() && self.color.is_none() && self.gradient_direction.is_none()
    }

    fn apply_to(&self, config: &mut NotificationConfig) {
        if let Some(value) = self.width {
            config.border.width = value;
        }
        if let Some(value) = &self.color {
            config.border.color = value.clone();
        }
        if let Some(value) = self.gradient_direction {
            config.border.gradient_direction = Some(value);
        }
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct ThumbnailOverride {
    pub size: Option<u32>,
    pub gap: Option<u32>,
}

impl ThumbnailOverride {
    fn is_empty(&self) -> bool {
        self.size.is_none() && self.gap.is_none()
    }

    fn apply_to(&self, config: &mut NotificationConfig) {
        if let Some(value) = self.size {
            config.thumbnail.size = value;
        }
        if let Some(value) = self.gap {
            config.thumbnail.gap = value;
        }
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct SummaryOverride {
    #[serde(flatten)]
    pub text: TextStyleOverride,
    pub bottom_gap: Option<f32>,
}

impl SummaryOverride {
    fn is_empty(&self) -> bool {
        self.text.is_empty() && self.bottom_gap.is_none()
    }

    fn apply_to(&self, config: &mut NotificationConfig) {
        self.text.apply_to(&mut config.summary.text);
        if let Some(value) = self.bottom_gap {
            config.summary.bottom_gap = value;
        }
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct TextStyleOverride {
    pub font_size: Option<f32>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_optional_rgba_color",
        serialize_with = "serialize_optional_rgba_color"
    )]
    pub color: Option<ColorConfig>,
    pub gradient_direction: Option<GradientDirection>,
    pub font_family: Option<String>,
}

impl TextStyleOverride {
    fn is_empty(&self) -> bool {
        self.font_size.is_none()
            && self.bold.is_none()
            && self.italic.is_none()
            && self.color.is_none()
            && self.gradient_direction.is_none()
            && self.font_family.is_none()
    }

    fn apply_to(&self, style: &mut TextStyleConfig) {
        if let Some(value) = self.font_size {
            style.font_size = value;
        }
        if let Some(value) = self.bold {
            style.bold = value;
        }
        if let Some(value) = self.italic {
            style.italic = value;
        }
        if let Some(value) = &self.color {
            style.color = value.clone();
        }
        if let Some(value) = self.gradient_direction {
            style.gradient_direction = Some(value);
        }
        if let Some(value) = &self.font_family {
            style.font_family = Some(value.clone());
        }
    }
}

pub type BodyOverride = TextStyleOverride;

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct ProgressOverride {
    pub direction: Option<ProgressDirection>,
    pub thickness: Option<ProgressThickness>,
    pub alignment: Option<ProgressAlignment>,
    pub inset: Option<u32>,
    pub corner_radius: Option<u32>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_optional_rgba_color",
        serialize_with = "serialize_optional_rgba_color"
    )]
    pub color: Option<ColorConfig>,
    pub gradient_direction: Option<GradientDirection>,
}

impl ProgressOverride {
    fn is_empty(&self) -> bool {
        self.direction.is_none()
            && self.thickness.is_none()
            && self.alignment.is_none()
            && self.inset.is_none()
            && self.corner_radius.is_none()
            && self.color.is_none()
            && self.gradient_direction.is_none()
    }

    fn apply_to(&self, config: &mut NotificationConfig) {
        if let Some(value) = self.direction {
            config.progress.direction = value;
        }
        if let Some(value) = &self.thickness {
            config.progress.thickness = value.clone();
        }
        if let Some(value) = self.alignment {
            config.progress.alignment = value;
        }
        if let Some(value) = self.inset {
            config.progress.inset = value;
        }
        if let Some(value) = self.corner_radius {
            config.progress.corner_radius = value;
        }
        if let Some(value) = &self.color {
            config.progress.color = value.clone();
        }
        if let Some(value) = self.gradient_direction {
            config.progress.gradient_direction = Some(value);
        }
    }
}

fn deserialize_optional_rgba_color<'de, D>(deserializer: D) -> Result<Option<ColorConfig>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_rgba_color(deserializer).map(Some)
}

fn serialize_optional_rgba_color<S>(
    color: &Option<ColorConfig>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match color {
        Some(color) => serialize_rgba_color(color, serializer),
        None => serializer.serialize_none(),
    }
}
