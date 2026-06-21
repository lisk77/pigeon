use config::{Config, File};
use serde::Deserialize;
use std::{
    collections::HashMap,
    env,
    path::{Path, PathBuf},
};

mod notification;
mod placement;
mod profiles;
mod timeouts;

pub use notification::NotificationConfig;
pub use placement::{PlacementConfig, Position};
pub use profiles::Profile;
pub use timeouts::TimeoutConfig;

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct PigeonConfig {
    pub timeouts: TimeoutConfig,
    pub placement: PlacementConfig,
    pub notification: NotificationConfig,
    #[serde(default)]
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
        let notification = &self.notification;
        if notification.min_width == 0 || notification.min_height == 0 {
            return Err(config::ConfigError::Message(
                "minimum card dimensions must be greater than zero".into(),
            ));
        }
        if notification.min_width > notification.max_width {
            return Err(config::ConfigError::Message(
                "notification.min_width must not exceed notification.max_width".into(),
            ));
        }
        if notification.min_height > notification.max_height {
            return Err(config::ConfigError::Message(
                "notification.min_height must not exceed notification.max_height".into(),
            ));
        }

        Ok(())
    }
}
