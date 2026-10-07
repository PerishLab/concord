use serde::{Deserialize, Serialize};

use crate::Result;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Target {
    Source,
    Release,
}

impl Target {
    pub fn label(self) -> &'static str {
        match self {
            Self::Source => "acceptance:source",
            Self::Release => "acceptance:release",
        }
    }

    pub fn select(labels: &[String]) -> Result<Option<Self>> {
        let mut selected = None;
        for label in labels
            .iter()
            .filter(|label| label.starts_with("acceptance:"))
        {
            let target = match label.as_str() {
                "acceptance:source" => Self::Source,
                "acceptance:release" => Self::Release,
                _ => return Err(super::fault("unknown managed acceptance target")),
            };
            if selected.replace(target).is_some() {
                return Err(super::fault("multiple managed acceptance target labels"));
            }
        }
        Ok(selected)
    }

    pub fn replace(self, labels: &[String]) -> Result<Vec<String>> {
        Self::select(labels)?;
        let mut retained: Vec<_> = labels
            .iter()
            .filter(|label| !label.starts_with("acceptance:"))
            .cloned()
            .collect();
        retained.push(self.label().into());
        Ok(retained)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Reference {
    pub node: String,
    pub digest: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Judgment {
    Satisfied,
    Unmet,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Verification {
    Manual,
    Machine,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Release {
    pub marker: String,
    pub distribution: String,
    pub inclusion: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "purpose", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Marker {
    Declaration {
        target: Target,
        promise: String,
    },
    Amendment {
        target: Target,
        promise: String,
        predecessor: Reference,
        reason: String,
    },
    Closure {
        target: Target,
        declaration: Reference,
        judgment: Judgment,
        verification: Verification,
        review: String,
        evidence: Vec<String>,
        remaining: Vec<String>,
        release: Option<Release>,
    },
}

impl Marker {
    pub fn target(&self) -> Target {
        match self {
            Self::Declaration { target, .. }
            | Self::Amendment { target, .. }
            | Self::Closure { target, .. } => *target,
        }
    }
}
