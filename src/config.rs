use config::{Config, File};
use serde::Deserialize;
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
        Config::builder()
            .add_source(File::from(path.as_ref()).required(false))
            .build()?
            .try_deserialize()
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
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct GeneralConfig {
    pub low_timeout: u64,
    pub normal_timeout: u64,
    pub position: Position,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            low_timeout: 3_000,
            normal_timeout: 5_000,
            position: Position::TopRight,
        }
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
