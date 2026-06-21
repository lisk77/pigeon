use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub struct Rule {
    pub action: RuleAction,
    pub app_name: Option<String>,
    pub desktop_entry: Option<String>,
    pub summary: Option<String>,
    pub body: Option<String>,
    pub category: Option<String>,
    pub stack_tag: Option<String>,
    pub urgency: Option<u8>,
    pub transient: Option<bool>,
    pub resident: Option<bool>,
    #[serde(default)]
    pub hints: HashMap<String, String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuleAction {
    #[default]
    Allow,
    Block,
}
