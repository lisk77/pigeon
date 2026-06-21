use config::{Config, File};
use serde::{Deserialize, Deserializer};
use std::{
    collections::HashMap,
    env,
    path::{Path, PathBuf},
};

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct PigeonConfig {
    pub general: GeneralConfig,
    profiles: HashMap<String, Profile>,
}

impl PigeonConfig {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, config::ConfigError> {
        let config: Self = Config::builder()
            .add_source(File::from(path.as_ref()).required(false))
            .build()?
            .try_deserialize()?;

        config.validate()?;
        Ok(config)
    }

    pub fn load_default() -> Result<Self, config::ConfigError> {
        Self::load(Self::default_path())
    }

    pub fn default_path() -> PathBuf {
        if let Some(path) = env::var_os("PIGEOND_CONFIG") {
            return path.into();
        }

        let config_home = env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
            .unwrap_or_else(|| PathBuf::from("."));

        config_home.join("pigeond/config.toml")
    }

    pub fn profile(&self, name: &str) -> Option<&Profile> {
        self.profiles.get(name)
    }

    fn validate(&self) -> Result<(), config::ConfigError> {
        let general = &self.general;
        if general.min_card_width == 0 || general.min_card_height == 0 {
            return Err(config::ConfigError::Message(
                "minimum card dimensions must be greater than zero".into(),
            ));
        }
        if general.min_card_width > general.max_card_width {
            return Err(config::ConfigError::Message(
                "min_card_width must not exceed max_card_width".into(),
            ));
        }
        if general.min_card_height > general.max_card_height {
            return Err(config::ConfigError::Message(
                "min_card_height must not exceed max_card_height".into(),
            ));
        }

        Ok(())
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct GeneralConfig {
    pub low_timeout: u64,
    pub normal_timeout: u64,
    pub position: Position,
    pub min_card_width: u32,
    pub max_card_width: u32,
    pub min_card_height: u32,
    pub max_card_height: u32,
    pub outer_padding: u32,
    pub thumbnail_size: u32,
    pub thumbnail_gap: u32,
    pub summary_font_size: f32,
    pub body_font_size: f32,
    pub summary_body_gap: f32,
    #[serde(deserialize_with = "deserialize_rgba_color")]
    pub background_color: [u8; 4],
    pub top_margin: u32,
    pub bottom_margin: u32,
    pub left_margin: u32,
    pub right_margin: u32,
    pub notification_gap: u32,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            low_timeout: 3_000,
            normal_timeout: 5_000,
            position: Position::TopRight,
            min_card_width: 240,
            max_card_width: 360,
            min_card_height: 96,
            max_card_height: 480,
            outer_padding: 16,
            thumbnail_size: 64,
            thumbnail_gap: 16,
            summary_font_size: 18.0,
            body_font_size: 14.0,
            summary_body_gap: 8.0,
            background_color: [0x20, 0x20, 0x20, 0xff],
            top_margin: 16,
            bottom_margin: 16,
            left_margin: 16,
            right_margin: 16,
            notification_gap: 8,
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

    // `wl_shm::Format::Argb8888` is stored as BGRA bytes on little-endian systems.
    Ok([blue, green, red, alpha])
}

#[cfg(test)]
mod tests {
    use super::parse_rgba_color;

    #[test]
    fn parses_rgb_as_opaque_rgba() {
        assert_eq!(parse_rgba_color("#a1b2c3"), Ok([0xc3, 0xb2, 0xa1, 0xff]));
    }

    #[test]
    fn parses_rgba() {
        assert_eq!(parse_rgba_color("#a1b2c380"), Ok([0xc3, 0xb2, 0xa1, 0x80]));
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Position {
    Top,
    TopLeft,
    #[default]
    TopRight,
    Bottom,
    BottomLeft,
    BottomRight,
    Left,
    Right,
}

#[derive(Debug, Deserialize)]
pub struct Profile {
    #[serde(default)]
    pub allowed: Vec<String>,
    #[serde(default)]
    pub blocked: Vec<String>,
}
