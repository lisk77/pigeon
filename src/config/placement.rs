use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct PlacementConfig {
    pub position: Position,
    pub top_margin: u32,
    pub bottom_margin: u32,
    pub left_margin: u32,
    pub right_margin: u32,
    pub notification_gap: u32,
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

impl Default for PlacementConfig {
    fn default() -> Self {
        Self {
            position: Position::TopRight,
            top_margin: 16,
            bottom_margin: 16,
            left_margin: 16,
            right_margin: 16,
            notification_gap: 8,
        }
    }
}
