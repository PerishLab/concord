use super::Issue;
use serde::Serialize;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReadinessChecks {
    pub required_sections: bool,
    pub acceptance_nonempty: bool,
    pub acceptance_settled: bool,
    pub acceptance_total: usize,
    pub acceptance_open: usize,
    pub sub_issues_closed: bool,
    pub blockers_closed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Readiness {
    pub schema: &'static str,
    pub issue: Issue,
    pub kind: String,
    pub ready: bool,
    pub checks: ReadinessChecks,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acceptance: Option<concord_core::acceptance::Evaluation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub automation: Option<concord_core::automation::Held>,
    pub reasons: Vec<String>,
    pub distribution_evidence: Vec<String>,
    pub observed_at: u64,
}
