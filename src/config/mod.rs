use config::{Config, File};
use serde::Deserialize;
use std::{
    collections::HashMap,
    env,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

use crate::notification::Notification;

pub mod notification;
mod profiles;
mod timeouts;

pub use notification::NotificationConfig;
pub use profiles::{Profile, ProfileConfig, RuleAction};
pub use timeouts::TimeoutConfig;

pub type SharedConfig = Arc<RwLock<PigeonConfig>>;

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct PigeonConfig {
    pub timeouts: TimeoutConfig,
    pub notification: NotificationConfig,
    pub profile: ProfileConfig,
    pub profiles: HashMap<String, Profile>,
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

    pub fn load_or_default(path: impl AsRef<Path>) -> Self {
        match Self::load(path) {
            Ok(config) => config,
            Err(error) => {
                eprintln!("config load failed; using defaults: {error}");
                Self::default()
            }
        }
    }

    pub fn load_startup() -> Self {
        Self::load_or_default(Self::default_path())
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

    pub fn selected_profile(&self, notification: &Notification) -> Option<&Profile> {
        let override_profile = self
            .profile
            .allow_profile_override
            .then(|| notification.profile())
            .flatten();

        override_profile
            .and_then(|name| self.profile(name))
            .or_else(|| self.profile(&self.profile.active))
    }

    pub fn presentation_for(
        &self,
        notification: &Notification,
    ) -> (RuleAction, NotificationConfig) {
        match self.selected_profile(notification) {
            Some(profile) => (
                profile.action_for(notification),
                profile.notification.apply_to(&self.notification),
            ),
            None => (RuleAction::Allow, self.notification.clone()),
        }
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

        if (self.profile.active != "default" || !self.profiles.is_empty())
            && !self.profiles.contains_key(&self.profile.active)
        {
            return Err(config::ConfigError::Message(format!(
                "active profile {:?} is not defined",
                self.profile.active
            )));
        }

        for (profile_name, profile) in &self.profiles {
            for (index, rule) in profile.rules.iter().enumerate() {
                if !rule.has_matcher() {
                    return Err(config::ConfigError::Message(format!(
                        "profiles.{profile_name}.rules[{index}] must have at least one matcher"
                    )));
                }
            }
        }

        Ok(())
    }
}
