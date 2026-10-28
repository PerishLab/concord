use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub owner: String,
    pub organization: String,
    pub field: String,
    pub options: BTreeMap<String, String>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Intent {
    pub reason: String,
    #[serde(deserialize_with = "choice")]
    pub value: Option<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Plan {
    pub schema: String,
    pub issue: String,
    pub binding: Binding,
    pub before: Value,
    pub intent: Intent,
}

pub(super) const NAMES: [&str; 4] = ["Clarify", "Deliver", "Verify", "Settle"];

fn choice<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Option<String>, D::Error> {
    Option::<String>::deserialize(deserializer)
}
