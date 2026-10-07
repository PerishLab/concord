use concord_core::acceptance::{Plan, Recovery};
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
}

impl Envelope {
    pub fn validate(&self, coordinate: &Coordinate) -> Result<()> {
        if self.version != 1 || self.plan.schema != "concord.acceptance-plan/v1" {
            return Err(super::fault("plan", "unsupported acceptance plan version"));
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
        self.plan
            .intent
            .recover(&snapshot.labels, &snapshot.comments())
    }
}
