use concord_core::acceptance::{Marker, Plan, Recovery, digest};
use concord_core::{Coordinate, Result};
use serde::{Deserialize, Serialize};

use super::super::forge::{self, Execution};
use super::model::{Header, Snapshot};

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Envelope {
    pub version: u32,
    pub plan: Prepared,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Prepared {
    pub schema: String,
    pub issue: Header,
    pub review: String,
    pub intent: Plan,
    pub execution: Execution,
    pub semantics: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authority: Option<Advice>,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum Status {
    Declared,
    Missing,
    Invalid,
    Unknown,
}

impl Status {
    pub fn message(self) -> &'static str {
        match self {
            Self::Declared => {
                "This repository declares a release authority; closure still requires one exact containing stable distribution and any promised consumer evidence."
            }
            Self::Missing => {
                "No release authority is currently declared in HEAD:plumb.toml. Retain future authority as an obligation, or explicitly amend a research or coordination promise to source while preserving each leaf's release obligations."
            }
            Self::Invalid => {
                "HEAD:plumb.toml does not declare a valid release authority; repair the declaration before release closure."
            }
            Self::Unknown => {
                "Release authority could not be read consistently for this repository; unreadable provider facts do not establish absence. Read current authority before release closure."
            }
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Advice {
    pub repository: String,
    pub manifest: Option<String>,
    pub status: Status,
    pub message: String,
}

impl Envelope {
    pub fn validate(&self, coordinate: &Coordinate) -> Result<()> {
        let versioned = match self.plan.schema.as_str() {
            "concord.acceptance-plan/v1" => self.plan.authority.is_none(),
            "concord.acceptance-plan/v2" => self.plan.authority.is_some(),
            _ => false,
        };
        if self.version != 1 || !versioned {
            return Err(super::fault("plan", "unsupported acceptance plan version"));
        }
        if let Some(advice) = &self.plan.authority {
            let identity = advice.manifest.as_ref().is_none_or(|value| {
                value.len() == 40
                    && value
                        .bytes()
                        .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
            });
            if advice.repository != self.plan.issue.repository
                || !identity
                || advice.message != advice.status.message()
            {
                return Err(super::fault(
                    "plan",
                    "authority advice differs from its repository or public shape",
                ));
            }
        }
        if &self.plan.issue.coordinate != coordinate {
            return Err(super::fault("plan", "plan belongs to another Issue"));
        }
        let encoded = serde_json::to_string(&self.plan.issue)
            .map_err(|error| super::fault("encode", error.to_string()))?;
        if concord_core::acceptance::digest(&encoded) != self.plan.review {
            return Err(super::fault(
                "plan",
                "plan review differs from its Issue basis",
            ));
        }
        let rendered = forge::render("probe", &self.plan.execution)?;
        let suffix = rendered.strip_prefix("probe").expect("rendered probe");
        if !self.plan.intent.body()?.ends_with(suffix) {
            return Err(super::fault(
                "plan",
                "plan execution footer differs from its body",
            ));
        }
        Ok(())
    }

    pub fn recover(&self, snapshot: &Snapshot) -> Result<Recovery> {
        let mut current = snapshot.header.clone();
        current.updated.clone_from(&self.plan.issue.updated);
        if current != self.plan.issue || current.state != "OPEN" {
            return Err(super::fault(
                "changed",
                "Issue identity, conditions or state changed",
            ));
        }
        let marker = Marker::parse(self.plan.intent.body()?)?
            .ok_or_else(|| super::fault("plan", "planned acceptance marker is missing"))?;
        review(&marker, &snapshot.header.body)?;
        self.plan
            .intent
            .recover(&snapshot.labels, &snapshot.comments())
    }
}

pub(super) fn review(marker: &Marker, body: &str) -> Result<()> {
    if let Marker::Closure { review, .. } = marker {
        let expected = digest(body);
        if *review != expected {
            return Err(super::fault(
                "review",
                format!(
                    "closure marker.review must be the SHA-256 of the exact current Issue body: {expected}; plan.review instead binds the plan Issue header; review the current conditions before preparing a new judgment"
                ),
            ));
        }
    }
    Ok(())
}
