pub(super) mod authority;
mod page;
mod readiness;

pub use readiness::{Checks, Readiness};

use concord_core::{Coordinate, Error, Result};
use serde::Serialize;
use std::time::Instant;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub(super) const BRIEF: &str = "concord.issue-brief/v1";
pub(super) const GRAPH: &str = "concord.issue-graph/v1";
pub(super) const READINESS: &str = "concord.issue-readiness/v1";

#[derive(Clone, Debug)]
pub struct Request {
    pub size: u16,
    pub children: Option<String>,
    pub blockers: Option<String>,
    pub blocking: Option<String>,
    pub pulls: Option<String>,
    pub comments: Option<String>,
}

impl Request {
    pub(super) fn first(size: u16) -> Self {
        Self {
            size,
            children: None,
            blockers: None,
            blocking: None,
            pulls: None,
            comments: None,
        }
    }
}

pub struct Projection {
    pub(super) size: u16,
    pub(super) pages: u16,
    pub(super) timeout: u64,
}

impl Projection {
    pub fn new(size: u16, pages: u16, timeout: u64) -> Self {
        Self {
            size,
            pages,
            timeout,
        }
    }

    pub async fn brief(&self, coordinate: &Coordinate, request: Request) -> Result<Brief> {
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
    raw: super::github::Snapshot,
    request: &Request,
) -> Result<Brief> {
    let issue = super::shape::root(coordinate, &raw).map_err(fault)?;
    let kind = raw
        .kind
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
    let automation = concord_core::automation::held(&kind, &raw.body, &issue.state);
    Ok(Brief {
        automation,
        schema: BRIEF,
        issue,
        kind,
        updated: raw.updated.clone(),
        parent: raw
            .parent
            .as_ref()
            .map(super::shape::issue)
            .transpose()
            .map_err(fault)?,
        summary: Summary {
            total: raw.summary.total,
            completed: raw.summary.completed,
            percent: raw.summary.percent,
        },
        children: super::shape::page(&raw.children, request.children.clone())?,
        blockers: super::shape::page(&raw.blockers, request.blockers.clone())?,
        blocking: super::shape::page(&raw.blocking, request.blocking.clone())?,
        pulls: super::shape::pulls(&raw.timeline, request.pulls.clone())?,
        observed: now(),
    })
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Issue {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub automation: Option<concord_core::automation::Held>,
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
    #[serde(rename = "merged_at")]
    pub merged: Option<String>,
    pub updated: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Page<T> {
    #[serde(rename = "requested_after")]
    pub after: Option<String>,
    #[serde(rename = "end_cursor")]
    pub cursor: Option<String>,
    #[serde(rename = "has_next_page")]
    pub more: bool,
    #[serde(rename = "provider_total")]
    pub total: usize,
    pub nodes: Vec<T>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Summary {
    pub total: usize,
    pub completed: usize,
    #[serde(rename = "percent_completed")]
    pub percent: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Brief {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub automation: Option<concord_core::automation::Held>,
    pub schema: &'static str,
    pub issue: Issue,
    pub kind: String,
    pub updated: String,
    pub parent: Option<Issue>,
    #[serde(rename = "sub_issue_summary")]
    pub summary: Summary,
    #[serde(rename = "sub_issues")]
    pub children: Page<Issue>,
    #[serde(rename = "blocked_by")]
    pub blockers: Page<Issue>,
    pub blocking: Page<Issue>,
    pub pulls: Page<Pull>,
    #[serde(rename = "observed_at")]
    pub observed: u64,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Link {
    Parent,
    Blocking,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct Edge {
    pub kind: Link,
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
    #[serde(rename = "observed_at")]
    pub observed: u64,
}

#[derive(Clone, Debug)]
pub(super) struct Complete {
    pub issue: Issue,
    pub kind: String,
    pub body: String,
    pub parent: Option<Issue>,
    pub children: Vec<Issue>,
    pub blockers: Vec<Issue>,
    pub blocking: Vec<Issue>,
    pub pulls: Vec<Pull>,
    pub comments: Vec<super::github::Comment>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum Connection {
    Children,
    Blockers,
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
