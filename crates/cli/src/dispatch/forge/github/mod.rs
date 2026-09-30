pub(super) mod transport;

use super::projection::{Fault, PageRequest, Projection};
use concord_core::Coordinate;
use serde::Deserialize;
use serde::de::DeserializeOwned;

const LIMIT: usize = 1024 * 1024;
const QUERY: &str = r#"
query(
  $owner: String!, $name: String!, $number: Int!, $first: Int!,
  $subAfter: String, $blockedAfter: String, $blockingAfter: String,
  $pullsAfter: String, $commentsAfter: String
) {
  repository(owner: $owner, name: $name) {
    issue(number: $number) {
      id number url title body state updatedAt
      issueType { name }
      repository { nameWithOwner }
      parent { id number url title state repository { nameWithOwner } }
      subIssues(first: $first, after: $subAfter) {
        totalCount pageInfo { hasNextPage endCursor }
        nodes { id number url title state repository { nameWithOwner } }
      }
      subIssuesSummary { total completed percentCompleted }
      blockedBy(first: $first, after: $blockedAfter) {
        totalCount pageInfo { hasNextPage endCursor }
        nodes { id number url title state repository { nameWithOwner } }
      }
      blocking(first: $first, after: $blockingAfter) {
        totalCount pageInfo { hasNextPage endCursor }
        nodes { id number url title state repository { nameWithOwner } }
      }
      timelineItems(first: $first, after: $pullsAfter, itemTypes: [CROSS_REFERENCED_EVENT]) {
        totalCount pageInfo { hasNextPage endCursor }
        nodes {
          ... on CrossReferencedEvent {
            source {
              __typename
              ... on PullRequest {
                id number url title state mergedAt updatedAt
                repository { nameWithOwner }
              }
            }
          }
        }
      }
      comments(first: $first, after: $commentsAfter) {
        totalCount pageInfo { hasNextPage endCursor }
        nodes { body url }
      }
    }
  }
}
"#;

#[derive(Deserialize)]
struct Envelope<T> {
    data: Option<T>,
    #[serde(default)]
    errors: Vec<GraphqlError>,
}

#[derive(Deserialize)]
struct GraphqlError {
    message: String,
}

#[derive(Deserialize)]
struct Data {
    repository: Option<RawRepository>,
}

#[derive(Deserialize)]
struct RawRepository {
    issue: Option<RawIssue>,
}

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

impl Projection<'_> {
    pub(super) async fn query(
        &self,
        coordinate: &Coordinate,
        request: &PageRequest,
    ) -> std::result::Result<RawIssue, Fault> {
        let process = self
            .request()
            .args(["api", "graphql", "-f"])
            .arg(format!("query={QUERY}"))
            .args(["-f", &format!("owner={}", coordinate.owner)])
            .args(["-f", &format!("name={}", coordinate.repository)])
            .args(["-F", &format!("number={}", coordinate.number)])
            .args(["-F", &format!("first={}", request.size)]);
        let process = cursors(process, request);
        decode(&self.run(process).await?)
    }

    pub(super) fn request(&self) -> transport::Request<'_> {
        transport::Request::new(self.command, self.timeout, LIMIT)
    }

    pub(super) async fn run(
        &self,
        process: transport::Request<'_>,
    ) -> std::result::Result<Vec<u8>, Fault> {
        let output = process.run().await.map_err(failure)?;
        Ok(output.stdout)
    }
}

fn cursors<'a>(
    mut process: transport::Request<'a>,
    request: &PageRequest,
) -> transport::Request<'a> {
    for (name, cursor) in [
        ("subAfter", request.sub_issues_after.as_deref()),
        ("blockedAfter", request.blocked_by_after.as_deref()),
        ("blockingAfter", request.blocking_after.as_deref()),
        ("pullsAfter", request.pulls_after.as_deref()),
        ("commentsAfter", request.comments_after.as_deref()),
    ] {
        if let Some(cursor) = cursor {
            process = process.args(["-f", &format!("{name}={cursor}")]);
        }
    }
    process
}

fn failure(failure: transport::Failure) -> Fault {
    use transport::{Failure, Stream};
    match failure {
        Failure::Spawn(_) | Failure::Exchange(_) => provider("command", failure.to_string()),
        Failure::Timeout => provider("timeout", "GitHub Issue projection timed out"),
        Failure::Refused { .. } => provider(
            "provider",
            "GitHub Issue projection was refused by the provider",
        ),
        Failure::Oversized(Stream::Stdout) => provider(
            "malformed",
            "GitHub Issue projection exceeds the reply limit",
        ),
        Failure::Oversized(Stream::Stderr) => provider(
            "provider",
            "GitHub Issue projection refusal exceeds the reply limit",
        ),
    }
}

fn decode(body: &[u8]) -> std::result::Result<RawIssue, Fault> {
    envelope::<Data>(body)?
        .and_then(|data| data.repository)
        .and_then(|repository| repository.issue)
        .ok_or_else(|| provider("missing", "GitHub Issue is not readable"))
}

pub(super) fn envelope<T: DeserializeOwned>(body: &[u8]) -> std::result::Result<Option<T>, Fault> {
    let envelope = serde_json::from_slice::<Envelope<T>>(body)
        .map_err(|_| provider("malformed", "GitHub Issue projection is not valid JSON"))?;
    if !envelope.errors.is_empty() {
        return Err(provider(
            "provider",
            envelope
                .errors
                .into_iter()
                .map(|error| error.message)
                .collect::<Vec<_>>()
                .join("; "),
        ));
    }
    Ok(envelope.data)
}

pub(super) fn provider(code: &'static str, message: impl Into<String>) -> Fault {
    Fault {
        code,
        message: message.into(),
    }
}
