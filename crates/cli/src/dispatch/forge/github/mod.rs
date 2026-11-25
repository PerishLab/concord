pub(crate) mod delivery;
pub(crate) mod identity;
pub(super) mod issues;
pub(super) mod lifecycle;
pub(crate) mod observation;
pub(super) mod projections;
pub(crate) mod pull;

use super::projection::{Fault, PageRequest, Projection};
use concord_core::Coordinate;
use serde::Deserialize;

pub(super) const LIMIT: usize = 1024 * 1024;

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RawIssue {
    pub id: String,
    pub number: i64,
    pub url: String,
    pub title: String,
    pub body: String,
    pub state: String,
    pub updated_at: String,
    pub issue_type: Option<RawType>,
    pub repository: RawName,
    pub parent: Option<RawIssueNode>,
    pub sub_issues: RawConnection<RawIssueNode>,
    pub sub_issues_summary: RawSubIssueSummary,
    pub blocked_by: RawConnection<RawIssueNode>,
    pub blocking: RawConnection<RawIssueNode>,
    pub timeline_items: RawConnection<RawEvent>,
    pub comments: RawConnection<RawComment>,
}

#[derive(Clone, Deserialize)]
pub(super) struct RawType {
    pub name: String,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RawIssueNode {
    pub id: String,
    pub number: i64,
    pub url: String,
    pub title: String,
    pub state: String,
    pub repository: RawName,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RawName {
    pub name_with_owner: String,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RawConnection<T> {
    pub total_count: usize,
    pub page_info: RawPageInfo,
    pub nodes: Vec<T>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RawPageInfo {
    pub has_next_page: bool,
    pub end_cursor: Option<String>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RawSubIssueSummary {
    pub total: usize,
    pub completed: usize,
    pub percent_completed: usize,
}

#[derive(Clone, Deserialize)]
pub(super) struct RawEvent {
    pub source: Option<RawSource>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RawSource {
    #[serde(rename = "__typename")]
    pub kind: String,
    pub id: Option<String>,
    pub number: Option<i64>,
    pub url: Option<String>,
    pub title: Option<String>,
    pub state: Option<String>,
    pub merged_at: Option<String>,
    pub updated_at: Option<String>,
    pub repository: Option<RawName>,
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct RawComment {
    pub body: String,
    pub url: String,
}

impl Projection {
    pub(super) async fn query(
        &self,
        coordinate: &Coordinate,
        request: &PageRequest,
    ) -> std::result::Result<RawIssue, Fault> {
        self.snapshot(coordinate, request)
    }
}

pub(super) fn provider(code: &'static str, message: impl Into<String>) -> Fault {
    Fault {
        code,
        message: message.into(),
    }
}
