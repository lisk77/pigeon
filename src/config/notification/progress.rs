use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::{ColorConfig, deserialize_rgba_color, serialize_rgba_color};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct ProgressConfig {
    pub direction: ProgressDirection,
    pub thickness: ProgressThickness,
    pub alignment: ProgressAlignment,
    pub inset: u32,
    pub corner_radius: u32,
    #[serde(
        deserialize_with = "deserialize_rgba_color",
        serialize_with = "serialize_rgba_color"
    )]
    pub color: ColorConfig,
}

impl Default for ProgressConfig {
    fn default() -> Self {
        Self {
            direction: ProgressDirection::LeftToRight,
            thickness: ProgressThickness::Percent(100.0),
            alignment: ProgressAlignment::Center,
            inset: 0,
            corner_radius: 0,
            color: ColorConfig::solid([0xac, 0x81, 0x5e, 0x80]),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProgressDirection {
    #[default]
    LeftToRight,
    RightToLeft,
    BottomToTop,
    TopToBottom,
}

impl ProgressDirection {
    pub fn is_horizontal(&self) -> bool {
        matches!(self, Self::LeftToRight | Self::RightToLeft)
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ProgressAlignment {
    Start,
    #[default]
    Center,
    End,
}

#[derive(Clone, Debug)]
pub enum ProgressThickness {
    Pixels(u32),
    Percent(f32),
}

impl ProgressThickness {
    pub fn resolve(&self, available: u32) -> u32 {
        match self {
            Self::Pixels(pixels) => (*pixels).min(available),
            Self::Percent(percent) => {
                ((available as f32 * percent / 100.0).round() as u32).min(available)
            }
        }
    }
}

impl<'de> Deserialize<'de> for ProgressThickness {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;

        if let Some(value) = value.strip_suffix("px") {
            return value.parse().map(Self::Pixels).map_err(|_| {
                serde::de::Error::custom("thickness must be a non-negative pixel value")
            });
        }

        if let Some(value) = value.strip_suffix('%') {
            let percent = value.parse::<f32>().map_err(|_| {
                serde::de::Error::custom("thickness must be a percentage such as 25%")
            })?;
            if !percent.is_finite() || !(0.0..=100.0).contains(&percent) {
                return Err(serde::de::Error::custom(
                    "percentage thickness must be between 0% and 100%",
                ));
            }
            return Ok(Self::Percent(percent));
        }

        Err(serde::de::Error::custom(
            "thickness must use px or % units, such as 5px or 100%",
        ))
    }
}

impl Serialize for ProgressThickness {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Pixels(value) => serializer.serialize_str(&format!("{value}px")),
            Self::Percent(value) => serializer.serialize_str(&format!("{value}%")),
        }
    }
}
