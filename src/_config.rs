#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct GeneralConfig {
    pub summary_font_size: f32,
    pub body_font_size: f32,
    pub summary_body_gap: f32,
    #[serde(deserialize_with = "deserialize_rgba_color")]
    pub background_color: [u8; 4],
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            summary_font_size: 18.0,
            body_font_size: 14.0,
            summary_body_gap: 8.0,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct Profile {
    #[serde(default)]
    pub allowed: Vec<String>,
    #[serde(default)]
    pub blocked: Vec<String>,
}
