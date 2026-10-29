use super::fault;
use concord_core::acceptance::{Evaluation, Reference, Target, Verdict};
use concord_core::{Coordinate, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(in crate::dispatch::forge) enum State {
    Open,
    Closed,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(in crate::dispatch::forge) enum Reason {
    Completed,
    #[serde(rename = "not-planned")]
    Declined,
    Duplicate,
    Reopened,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(in crate::dispatch::forge) struct Issue {
    pub node: String,
    pub repository: String,
    pub url: String,
    pub kind: String,
    pub state: State,
    pub reason: Option<Reason>,
    pub review: String,
    pub updated: String,
    pub writable: bool,
}

impl Issue {
    pub fn ordinary(&self) -> Result<()> {
        if !matches!(self.kind.as_str(), "Bug" | "Feature" | "Task") {
            return Err(fault(
                "type",
                "only known ordinary Issue types can use explicit closure; Auto and unknown types remain read-only",
            ));
        }
        Ok(())
    }

    pub fn completed(&self) -> bool {
        self.state == State::Closed && self.reason == Some(Reason::Completed)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Basis {
    pub issue: Issue,
    pub labels: Vec<String>,
    pub target: Target,
    pub declaration: Reference,
    pub closure: Reference,
}

#[derive(Serialize)]
pub(super) struct Observation {
    pub issue: Issue,
    pub labels: Vec<String>,
    pub acceptance: Evaluation,
}

impl Observation {
    pub fn basis(&self) -> Result<Basis> {
        self.issue.ordinary()?;
        if self.labels.iter().any(|label| label.starts_with("needs:")) {
            return Err(fault(
                "needs",
                format!(
                    "resolve the operated Issue's needs labels before closing it: {}",
                    self.labels
                        .iter()
                        .filter(|label| label.starts_with("needs:"))
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            ));
        }
        if self.acceptance.verdict != Verdict::Satisfied {
            return Err(super::diagnostic::refused(&self.acceptance));
        }
        Ok(Basis {
            issue: self.issue.clone(),
            labels: self.labels.clone(),
            target: self
                .acceptance
                .target
                .ok_or_else(|| fault("acceptance", "an adopted acceptance target is required"))?,
            declaration: self
                .acceptance
                .declaration
                .clone()
                .ok_or_else(|| fault("acceptance", "current declaration is required"))?,
            closure: self
                .acceptance
                .closure
                .clone()
                .ok_or_else(|| fault("acceptance", "current closure judgment is required"))?,
        })
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Envelope {
    pub version: u8,
    pub plan: Plan,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Plan {
    pub schema: String,
    pub issue: Coordinate,
    pub basis: Basis,
}

impl Envelope {
    pub fn validate(&self, coordinate: &Coordinate) -> Result<()> {
        let identity = self.version == 1
            && self.plan.schema == "concord.issue-close-plan/v1"
            && &self.plan.issue == coordinate;
        let writable = self.plan.basis.issue.state == State::Open && self.plan.basis.issue.writable;
        if !identity || !writable {
            return Err(fault(
                "plan",
                "unsupported, foreign or non-open closure plan; no write was attempted",
            ));
        }
        self.plan.basis.issue.ordinary()
    }
}

impl Plan {
    pub fn contains(&self, current: &Basis) -> bool {
        let mut normalized = current.clone();
        normalized.issue.state = self.basis.issue.state;
        normalized.issue.reason = self.basis.issue.reason;
        normalized.issue.updated = self.basis.issue.updated.clone();
        normalized == self.basis
    }
}
