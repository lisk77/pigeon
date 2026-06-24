use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct TimeoutConfig {
    pub low: u64,
    pub normal: u64,
    #[serde(
        deserialize_with = "deserialize",
        serialize_with = "serialize"
    )]
    pub critical: u64,
}

fn deserialize<'de, D>(deserializer: D) -> Result<u64, D::Error>
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

fn serialize<S>(timeout: &u64, serializer: S) -> Result<S::Ok, S::Error>
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
            low: 3000,
            normal: 5000,
            critical: u64::MAX,
        }
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct TimeoutOverride {
    pub low: Option<u64>,
    pub normal: Option<u64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_optional",
        serialize_with = "serialize_optional"
    )]
    pub critical: Option<u64>,
}

impl TimeoutOverride {
    pub(crate) fn is_empty(&self) -> bool {
        self.low.is_none()
            && self.normal.is_none()
            && self.critical.is_none()
    }

    pub fn apply_to(&self, base: &TimeoutConfig) -> TimeoutConfig {
        let mut resolved = base.clone();
        if let Some(value) = self.low {
            resolved.low = value;
        }
        if let Some(value) = self.normal {
            resolved.normal = value;
        }
        if let Some(value) = self.critical {
            resolved.critical = value;
        }
        resolved
    }
}

fn deserialize_optional<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
where
    D: Deserializer<'de>,
{
    deserialize(deserializer).map(Some)
}

fn serialize_optional<S>(timeout: &Option<u64>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match timeout {
        Some(timeout) => serialize(timeout, serializer),
        None => serializer.serialize_none(),
    }
}
