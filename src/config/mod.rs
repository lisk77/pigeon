use config::{Config, File};
use serde::{Deserialize, Serialize};
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
pub use timeouts::{TimeoutConfig, TimeoutOverride};

pub type SharedConfig = Arc<RwLock<PigeonConfig>>;

#[derive(Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct PigeonConfig {
    #[serde(skip)]
    pub path: PathBuf,
    pub timeouts: TimeoutConfig,
    pub notification: NotificationConfig,
    pub profile: ProfileConfig,
    #[serde(skip_serializing_if = "profiles_are_implicit")]
    pub profiles: HashMap<String, Profile>,
}

fn profiles_are_implicit(profiles: &HashMap<String, Profile>) -> bool {
    profiles.len() == 1 && profiles.get("default").is_some_and(Profile::is_default)
}

impl Default for PigeonConfig {
    fn default() -> Self {
        let mut profiles = HashMap::new();
        profiles.insert("default".into(), Profile::default());

        Self {
            path: Self::default_path(),
            timeouts: TimeoutConfig::default(),
            notification: NotificationConfig::default(),
            profile: ProfileConfig::default(),
            profiles,
        }
    }
}

impl PigeonConfig {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, config::ConfigError> {
        let path = path.as_ref().to_path_buf();
        let mut config: Self = Config::builder()
            .add_source(File::from(path.as_path()).required(false))
            .build()?
            .try_deserialize()?;

        config.path = path;
        config.profiles.entry("default".into()).or_default();
        config.validate()?;
        Ok(config)
    }

    pub fn load_default() -> Result<Self, config::ConfigError> {
        Self::load(Self::default_path())
    }

    pub fn default_toml() -> Result<String, toml::ser::Error> {
        toml::to_string_pretty(&Self::default())
    }

    pub fn load_or_default(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref().to_path_buf();
        match Self::load(&path) {
            Ok(config) => config,
            Err(error) => {
                eprintln!("config load failed; using defaults: {error}");
                Self {
                    path,
                    ..Self::default()
                }
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

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn profile(&self, name: &str) -> Option<&Profile> {
        self.profiles.get(name)
    }

    pub fn presentation_for(
        &self,
        notification: &Notification,
    ) -> (RuleAction, NotificationConfig, TimeoutConfig) {
        if self.profile.allow_profile_override {
            if let Some(name) = notification.profile() {
                if let Some(presentation) = self.profile_presentation(name, notification) {
                    return presentation;
                }
            }
        }

        self.profile_presentation(&self.profile.active, notification)
            .expect("active profile validated during config load")
    }

    fn profile_presentation(
        &self,
        name: &str,
        notification: &Notification,
    ) -> Option<(RuleAction, NotificationConfig, TimeoutConfig)> {
        let default_profile = self.profile("default")?;
        let default_style = default_profile.notification.apply_to(&self.notification);
        let default_timeouts = default_profile.timeouts.apply_to(&self.timeouts);
        let profile = self.profile(name)?;

        let (profile_style, profile_timeouts) = if name == "default" {
            (default_style, default_timeouts)
        } else {
            (
                profile.notification.apply_to(&default_style),
                profile.timeouts.apply_to(&default_timeouts),
            )
        };

        match profile.matching_rule(notification) {
            Some(rule) => Some((
                rule.action.unwrap_or(profile.default_action),
                rule.notification.apply_to(&profile_style),
                profile_timeouts,
            )),
            None => Some((profile.default_action, profile_style, profile_timeouts)),
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

        if !self.profiles.contains_key(&self.profile.active) {
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
