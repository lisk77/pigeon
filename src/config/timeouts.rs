use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct TimeoutConfig {
    pub low_timeout: u64,
    pub normal_timeout: u64,
    pub critical_timeout: u64,
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
