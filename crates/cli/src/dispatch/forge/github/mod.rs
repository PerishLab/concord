pub(crate) mod delivery;
pub(crate) mod identity;
pub(super) mod issues;
pub(super) mod lifecycle;
pub(crate) mod observation;
pub(super) mod projections;
pub(crate) mod pull;

use super::projection::{Fault, Projection, Request};
use concord_core::Coordinate;
use serde::Deserialize;

pub(super) const LIMIT: usize = 1024 * 1024;

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Snapshot {
    pub id: String,
    pub number: i64,
    pub url: String,
    pub title: String,
    pub body: String,
    pub state: String,
    #[serde(rename = "updatedAt")]
    pub updated: String,
    #[serde(rename = "issueType")]
    pub kind: Option<Kind>,
    pub repository: Name,
    pub parent: Option<Node>,
    #[serde(rename = "subIssues")]
    pub children: Connection<Node>,
    #[serde(rename = "subIssuesSummary")]
    pub summary: Summary,
    #[serde(rename = "blockedBy")]
    pub blockers: Connection<Node>,
    pub blocking: Connection<Node>,
    #[serde(rename = "timelineItems")]
    pub timeline: Connection<Event>,
    pub comments: Connection<Comment>,
}

#[derive(Clone, Deserialize)]
pub(super) struct Kind {
    pub name: String,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Node {
    pub id: String,
    pub number: i64,
    pub url: String,
    pub title: String,
    pub state: String,
    pub repository: Name,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Name {
    #[serde(rename = "nameWithOwner")]
    pub full: String,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Connection<T> {
    #[serde(rename = "totalCount")]
    pub total: usize,
    #[serde(rename = "pageInfo")]
    pub info: Paging,
    pub nodes: Vec<T>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Paging {
    #[serde(rename = "hasNextPage")]
    pub more: bool,
    #[serde(rename = "endCursor")]
    pub cursor: Option<String>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Summary {
    pub total: usize,
    pub completed: usize,
    #[serde(rename = "percentCompleted")]
    pub percent: usize,
}

#[derive(Clone, Deserialize)]
pub(super) struct Event {
    pub source: Option<Source>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Source {
    #[serde(rename = "__typename")]
    pub kind: String,
    pub id: Option<String>,
    pub number: Option<i64>,
    pub url: Option<String>,
    pub title: Option<String>,
    pub state: Option<String>,
    #[serde(rename = "mergedAt")]
    pub merged: Option<String>,
    #[serde(rename = "updatedAt")]
    pub updated: Option<String>,
    pub repository: Option<Name>,
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct Comment {
    pub body: String,
    pub url: String,
}

impl Projection {
    pub(super) async fn query(
        &self,
        coordinate: &Coordinate,
        request: &Request,
    ) -> std::result::Result<Snapshot, Fault> {
        self.snapshot(coordinate, request)
    }
}

pub(super) fn provider(code: &'static str, message: impl Into<String>) -> Fault {
    Fault {
        code,
        message: message.into(),
    }
}
