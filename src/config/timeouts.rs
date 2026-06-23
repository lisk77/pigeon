use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct TimeoutConfig {
    pub low_timeout: u64,
    pub normal_timeout: u64,
    #[serde(
        deserialize_with = "deserialize_timeout",
        serialize_with = "serialize_timeout"
    )]
    pub critical_timeout: u64,
}

fn deserialize_timeout<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum TimeoutValue {
        Milliseconds(u64),
        Never(String),
    }

    match TimeoutValue::deserialize(deserializer)? {
        TimeoutValue::Milliseconds(value) => Ok(value),
        TimeoutValue::Never(value) if value == "never" => Ok(u64::MAX),
        TimeoutValue::Never(_) => Err(serde::de::Error::custom(
            "timeout must be milliseconds or \"never\"",
        )),
    }
}

fn serialize_timeout<S>(timeout: &u64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    if *timeout == u64::MAX {
        serializer.serialize_str("never")
    } else {
        serializer.serialize_u64(*timeout)
    }
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            low_timeout: 3000,
            normal_timeout: 5000,
            critical_timeout: u64::MAX,
        }
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct TimeoutOverride {
    pub low_timeout: Option<u64>,
    pub normal_timeout: Option<u64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_optional_timeout",
        serialize_with = "serialize_optional_timeout"
    )]
    pub critical_timeout: Option<u64>,
}

impl TimeoutOverride {
    pub(crate) fn is_empty(&self) -> bool {
        self.low_timeout.is_none()
            && self.normal_timeout.is_none()
            && self.critical_timeout.is_none()
    }

    pub fn apply_to(&self, base: &TimeoutConfig) -> TimeoutConfig {
        let mut resolved = base.clone();
        if let Some(value) = self.low_timeout {
            resolved.low_timeout = value;
        }
        if let Some(value) = self.normal_timeout {
            resolved.normal_timeout = value;
        }
        if let Some(value) = self.critical_timeout {
            resolved.critical_timeout = value;
        }
        resolved
    }
}

fn deserialize_optional_timeout<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize_timeout(deserializer).map(Some)
}

fn serialize_optional_timeout<S>(timeout: &Option<u64>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match timeout {
        Some(timeout) => serialize_timeout(timeout, serializer),
        None => serializer.serialize_none(),
    }
}
