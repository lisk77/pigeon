use serde::{Deserialize, Deserializer};

mod body;
mod summary;
mod thumbnail;

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct NotificationConfig {
    pub min_width: u32,
    pub max_width: u32,
    pub min_height: u32,
    pub max_height: u32,
    pub outer_padding: u32,
    pub corner_radius: u32,
    #[serde(deserialize_with = "deserialize_rgba_color")]
    pub background_color: [u8; 4],
    pub thumbnail: thumbnail::ThumbnailConfig,
    pub summary: summary::SummaryConfig,
    pub body: body::BodyConfig,
}

impl Default for NotificationConfig {
    fn default() -> Self {
        Self {
            min_width: 240,
            max_width: 360,
            min_height: 96,
            max_height: 480,
            outer_padding: 16,
            corner_radius: 12,
            background_color: [0x20, 0x20, 0x20, 0xff],
            thumbnail: thumbnail::ThumbnailConfig::default(),
            summary: summary::SummaryConfig::default(),
            body: body::BodyConfig::default(),
        }
    }
}

fn deserialize_rgba_color<'de, D>(deserializer: D) -> Result<[u8; 4], D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    parse_rgba_color(&value).map_err(serde::de::Error::custom)
}

fn parse_rgba_color(value: &str) -> Result<[u8; 4], String> {
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
