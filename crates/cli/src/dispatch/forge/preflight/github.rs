use super::Diagnostic;
use serde::Deserialize;
use std::collections::BTreeMap;

const LIMIT: usize = 1024 * 1024;
const QUERY: &str = r#"
query($queryString: String!, $first: Int!, $after: String) {
  search(query: $queryString, type: ISSUE, first: $first, after: $after) {
    issueCount pageInfo { hasNextPage endCursor }
    nodes { ... on Issue {
      id number url title body state updatedAt issueType { name }
      repository { nameWithOwner }
    } }
  }
}
"#;

pub(super) struct Request<'a> {
    pub bounds: &'a super::Bounds<'a>,
    pub query: &'a str,
}

pub(super) struct Search {
    pub issues: Vec<RawIssue>,
    pub diagnostics: Vec<Diagnostic>,
    pub complete: bool,
}

#[derive(Deserialize)]
struct Envelope {
    data: Option<Data>,
    #[serde(default)]
    errors: Vec<GraphqlError>,
}

#[derive(Deserialize)]
struct GraphqlError {
    message: String,
}

#[derive(Deserialize)]
struct Data {
    search: RawSearch,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawSearch {
    issue_count: usize,
    page_info: RawPageInfo,
    nodes: Vec<RawIssue>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawPageInfo {
    has_next_page: bool,
    end_cursor: Option<String>,
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
    pub repository: RawRepository,
}

#[derive(Clone, Deserialize)]
pub(super) struct RawType {
    pub name: String,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RawRepository {
    pub name_with_owner: String,
}

pub(super) async fn search(request: Request<'_>) -> Search {
    let mut issues = BTreeMap::new();
    let mut diagnostics = Vec::new();
    let mut cursor = None;
    let mut complete = true;
    for turn in 0..request.bounds.pages {
        let found = page(&request, cursor.as_deref()).await;
        let found = match found {
            Ok(found) => found,
            Err(fault) => {
                complete = false;
                diagnostics.push(fault);
                break;
            }
        };
        let more = found.page_info.has_next_page;
        cursor = found.page_info.end_cursor;
        for issue in found.nodes {
            issues.insert(issue.id.clone(), issue);
        }
        if !more {
            break;
        }
        if turn + 1 == request.bounds.pages || cursor.is_none() {
            complete = false;
            diagnostics.push(Diagnostic {
                code: "truncated".to_string(),
                message: format!(
                    "GitHub search has more results after {} bounded pages",
                    request.bounds.pages
                ),
            });
        }
    }
    Search {
        issues: issues.into_values().collect(),
        diagnostics,
        complete,
    }
}

async fn page(request: &Request<'_>, after: Option<&str>) -> Result<RawSearch, Diagnostic> {
    let mut process = super::super::github::transport::Request::new(
        request.bounds.command,
        request.bounds.timeout,
        LIMIT,
    )
    .args(["api", "graphql", "-f"])
    .arg(format!("query={QUERY}"))
    .args(["-f", &format!("queryString={}", request.query)])
    .args(["-F", &format!("first={}", request.bounds.first)]);
    if let Some(after) = after {
        process = process.args(["-f", &format!("after={after}")]);
    }
    let output = process.run().await.map_err(search_failure)?;
    decode(&output.stdout)
}

fn search_failure(failure: super::super::github::transport::Failure) -> Diagnostic {
    use super::super::github::transport::{Failure, Stream};
    match failure {
        Failure::Spawn(_) | Failure::Exchange(_) => fault("command", failure.to_string()),
        Failure::Timeout => fault("timeout", "GitHub Issue search timed out"),
        Failure::Refused { .. } => fault(
            "provider",
            "GitHub Issue search was refused by the provider",
        ),
        Failure::Oversized(Stream::Stdout) => {
            fault("malformed", "GitHub Issue search exceeds the reply limit")
        }
        Failure::Oversized(Stream::Stderr) => fault(
            "provider",
            "GitHub Issue search refusal exceeds the reply limit",
        ),
    }
}

fn decode(body: &[u8]) -> Result<RawSearch, Diagnostic> {
    let envelope = serde_json::from_slice::<Envelope>(body)
        .map_err(|_| fault("malformed", "GitHub Issue search is not valid JSON"))?;
    if !envelope.errors.is_empty() {
        return Err(fault(
            "provider",
            envelope
                .errors
                .into_iter()
                .map(|error| error.message)
                .collect::<Vec<_>>()
                .join("; "),
        ));
    }
    let search = envelope
        .data
        .map(|data| data.search)
        .ok_or_else(|| fault("malformed", "GitHub Issue search has no data"))?;
    let _ = search.issue_count;
    Ok(search)
}

fn fault(code: &str, message: impl Into<String>) -> Diagnostic {
    Diagnostic {
        code: code.to_string(),
        message: message.into(),
    }
}
