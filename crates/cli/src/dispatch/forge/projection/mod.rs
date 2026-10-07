pub(super) mod authority;
mod page;

use concord_core::{Coordinate, Error, Result};
use serde::Serialize;
use std::path::Path;
use std::time::Instant;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub(super) const BRIEF: &str = "concord.issue-brief/v1";
pub(super) const GRAPH: &str = "concord.issue-graph/v1";
pub(super) const READINESS: &str = "concord.issue-readiness/v1";

#[derive(Clone, Debug)]
pub struct PageRequest {
    pub size: u16,
    pub sub_issues_after: Option<String>,
    pub blocked_by_after: Option<String>,
    pub blocking_after: Option<String>,
    pub pulls_after: Option<String>,
    pub comments_after: Option<String>,
}

impl PageRequest {
    pub(super) fn first(size: u16) -> Self {
        Self {
            size,
            sub_issues_after: None,
            blocked_by_after: None,
            blocking_after: None,
            pulls_after: None,
            comments_after: None,
        }
    }
}

pub struct Projection<'a> {
    pub(super) command: &'a Path,
    pub(super) page_size: u16,
    pub(super) max_pages: u16,
    pub(super) timeout: u64,
}

impl<'a> Projection<'a> {
    pub fn new(command: &'a Path, page_size: u16, max_pages: u16, timeout: u64) -> Self {
        Self {
            command,
            page_size,
            max_pages,
            timeout,
        }
    }

    pub async fn brief(&self, coordinate: &Coordinate, request: PageRequest) -> Result<Brief> {
        let started = Instant::now();
        let reply = self.query(coordinate, &request).await;
        match reply {
            Ok(raw) => match brief(coordinate, raw, &request) {
                Ok(brief) => {
                    record("issue.brief", started.elapsed(), "fresh");
                    Ok(brief)
                }
                Err(error) => {
                    record("issue.brief", started.elapsed(), error.code());
                    Err(error)
                }
            },
            Err(error) => {
                record("issue.brief", started.elapsed(), error.code);
                Err(fault(error))
            }
        }
    }
}

fn brief(
    coordinate: &Coordinate,
    raw: super::github::RawIssue,
    request: &PageRequest,
) -> Result<Brief> {
    let issue = super::shape::root(coordinate, &raw).map_err(fault)?;
    let kind = raw
        .issue_type
        .as_ref()
        .map(|kind| kind.name.trim())
        .filter(|kind| !kind.is_empty())
        .ok_or_else(|| {
            Error::typed(
                "concord.issue.projection.type",
                "GitHub Issue has no enabled native type",
            )
        })?
        .to_string();
    Ok(Brief {
        schema: BRIEF,
        issue,
        kind,
        updated: raw.updated_at.clone(),
        parent: raw
            .parent
            .as_ref()
            .map(super::shape::issue)
            .transpose()
            .map_err(fault)?,
        sub_issue_summary: SubIssueSummary {
            total: raw.sub_issues_summary.total,
            completed: raw.sub_issues_summary.completed,
            percent_completed: raw.sub_issues_summary.percent_completed,
        },
        sub_issues: super::shape::page(&raw.sub_issues, request.sub_issues_after.clone())?,
        blocked_by: super::shape::page(&raw.blocked_by, request.blocked_by_after.clone())?,
        blocking: super::shape::page(&raw.blocking, request.blocking_after.clone())?,
        pulls: super::shape::pulls(&raw.timeline_items, request.pulls_after.clone())?,
        observed_at: now(),
    })
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Issue {
    pub node: String,
    #[serde(flatten)]
    pub coordinate: Coordinate,
    pub url: String,
    pub title: String,
    pub state: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Pull {
    pub node: String,
    pub owner: String,
    pub repository: String,
    pub number: i64,
    pub url: String,
    pub title: String,
    pub state: String,
    pub merged_at: Option<String>,
    pub updated: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Page<T> {
    pub requested_after: Option<String>,
    pub end_cursor: Option<String>,
    pub has_next_page: bool,
    pub provider_total: usize,
    pub nodes: Vec<T>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SubIssueSummary {
    pub total: usize,
    pub completed: usize,
    pub percent_completed: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Brief {
    pub schema: &'static str,
    pub issue: Issue,
    pub kind: String,
    pub updated: String,
    pub parent: Option<Issue>,
    pub sub_issue_summary: SubIssueSummary,
    pub sub_issues: Page<Issue>,
    pub blocked_by: Page<Issue>,
    pub blocking: Page<Issue>,
    pub pulls: Page<Pull>,
    pub observed_at: u64,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum EdgeKind {
    Parent,
    Blocking,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct Edge {
    pub kind: EdgeKind,
    pub source: String,
    pub target: String,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct Diagnostic {
    pub code: String,
    pub subject: String,
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Graph {
    pub schema: &'static str,
    pub root: Coordinate,
    pub nodes: Vec<Issue>,
    pub edges: Vec<Edge>,
    pub diagnostics: Vec<Diagnostic>,
    pub complete: bool,
    pub observed_at: u64,
}

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
    pub acceptance: concord_core::acceptance::Evaluation,
    pub reasons: Vec<String>,
    pub distribution_evidence: Vec<String>,
    pub observed_at: u64,
}

#[derive(Clone, Debug)]
pub(super) struct CompleteIssue {
    pub issue: Issue,
    pub kind: String,
    pub body: String,
    pub parent: Option<Issue>,
    pub sub_issues: Vec<Issue>,
    pub blocked_by: Vec<Issue>,
    pub blocking: Vec<Issue>,
    pub pulls: Vec<Pull>,
    pub comments: Vec<super::github::RawComment>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum Connection {
    SubIssues,
    BlockedBy,
    Blocking,
    Pulls,
    Comments,
}

#[derive(Debug)]
pub(super) struct Fault {
    pub code: &'static str,
    pub message: String,
}

pub(super) fn key(coordinate: &Coordinate) -> String {
    format!(
        "{}/{}#{}",
        coordinate.owner, coordinate.repository, coordinate.number
    )
}

pub(super) fn fault(error: Fault) -> Error {
    Error::typed(
        format!("concord.issue.projection.{}", error.code),
        error.message,
    )
}

pub(super) fn record(shape: &str, duration: Duration, outcome: &str) {
    crate::observation::projection(
        shape,
        ("github", "issue"),
        u64::try_from(duration.as_millis()).unwrap_or(u64::MAX),
        outcome,
    );
}

pub(super) fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
