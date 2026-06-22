use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct PositionConfig {
    pub anchor: Anchor,
    pub top_margin: u32,
    pub bottom_margin: u32,
    pub left_margin: u32,
    pub right_margin: u32,
    pub notification_gap: u32,
}

#[derive(Clone, Copy, Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Anchor {
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

impl Default for PositionConfig {
    fn default() -> Self {
        Self {
            anchor: Anchor::TopRight,
            top_margin: 16,
            bottom_margin: 0,
            left_margin: 0,
            right_margin: 16,
            notification_gap: 8,
        }
    }
}
