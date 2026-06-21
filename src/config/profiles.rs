use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Profile {
    #[serde(default)]
    pub allowed: Vec<String>,
    #[serde(default)]
    pub blocked: Vec<String>,
}
