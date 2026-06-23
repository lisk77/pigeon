use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Deserialize, Serialize)]
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
