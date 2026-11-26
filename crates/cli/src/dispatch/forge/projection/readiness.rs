use super::Issue;
use serde::Serialize;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Checks {
    #[serde(rename = "required_sections")]
    pub sections: bool,
    #[serde(rename = "acceptance_nonempty")]
    pub nonempty: bool,
    #[serde(rename = "acceptance_settled")]
    pub settled: bool,
    #[serde(rename = "acceptance_total")]
    pub total: usize,
    #[serde(rename = "acceptance_open")]
    pub open: usize,
    #[serde(rename = "sub_issues_closed")]
    pub children: bool,
    #[serde(rename = "blockers_closed")]
    pub blockers: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Readiness {
    pub schema: &'static str,
    pub issue: Issue,
    pub kind: String,
    pub ready: bool,
    pub checks: Checks,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acceptance: Option<concord_core::acceptance::Evaluation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub automation: Option<concord_core::automation::Held>,
    pub reasons: Vec<String>,
    #[serde(rename = "distribution_evidence")]
    pub evidence: Vec<String>,
    #[serde(rename = "observed_at")]
    pub observed: u64,
}
