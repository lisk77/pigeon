use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct AnimationConfig {
    pub enabled: bool,
    pub direction: AnimationDirection,
    pub enter: TransitionConfig,
    pub exit: TransitionConfig,
}

impl Default for AnimationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            direction: AnimationDirection::Anchor,
            enter: TransitionConfig::default(),
            exit: TransitionConfig::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum AnimationDirection {
    Anchor,
    Top,
    Right,
    Bottom,
    Left,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct TransitionConfig {
    pub effect: AnimationEffect,
    pub duration: u64,
}

impl Default for TransitionConfig {
    fn default() -> Self {
        Self {
            effect: AnimationEffect::Slide,
            duration: 180,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum AnimationEffect {
    None,
    Slide,
}
