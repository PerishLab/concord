use super::github::RawIssue;
use super::{Bounds, Diagnostic};
use concord_core::{Coordinate, Estate, IssueWorktree, automation};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::Path;

const QUERY: &str = r#"
query($owner: String!, $repo: String!, $first: Int!, $after: String) {
  repository(owner: $owner, name: $repo) {
    issues(states: [OPEN], first: $first, after: $after) {
      nodes { id number url title body state updatedAt issueType { name }
        repository { nameWithOwner } }
      pageInfo { hasNextPage endCursor }
    }
  }
}
"#;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Observation {
    pub issue: Coordinate,
    pub automation: automation::Held,
    pub member: Coordinate,
    pub branch: String,
    pub exclusive: bool,
    pub paths: Option<Vec<String>>,
}

pub(super) struct Observed {
    pub observations: Vec<Observation>,
    pub diagnostics: Vec<Diagnostic>,
    pub complete: bool,
}

#[derive(Deserialize)]
struct Envelope {
    data: Option<Data>,
    #[serde(default)]
    errors: Vec<Failure>,
}

#[derive(Deserialize)]
struct Failure {
    message: String,
}

#[derive(Deserialize)]
struct Data {
    repository: Repository,
}

#[derive(Deserialize)]
struct Repository {
    issues: Page,
}

#[derive(Deserialize)]
struct Page {
    nodes: Vec<RawIssue>,
    #[serde(rename = "pageInfo")]
    page: Cursor,
}

#[derive(Deserialize)]
struct Cursor {
    #[serde(rename = "hasNextPage")]
    more: bool,
    #[serde(rename = "endCursor")]
    end: Option<String>,
}

pub(super) async fn observe(
    estate: &Estate,
    coordinate: &Coordinate,
    bounds: &Bounds<'_>,
    members: &[IssueWorktree],
) -> Observed {
    let mut observed = Observed {
        observations: Vec::new(),
        diagnostics: Vec::new(),
        complete: true,
    };
    let members = members
        .iter()
        .filter(|member| {
            member.issue.owner.eq_ignore_ascii_case(&coordinate.owner)
                && member
                    .issue
                    .repository
                    .eq_ignore_ascii_case(&coordinate.repository)
        })
        .collect::<Vec<_>>();
    if members.is_empty() {
        return observed;
    }
    let paths = estate
        .integration(coordinate)
        .await
        .and_then(|integration| automation::paths(Path::new(&integration.path)));
    let paths = match paths {
        Ok(paths) => paths,
        Err(error) => {
            observed.complete = false;
            observed.diagnostics.push(fault(error));
            return observed;
        }
    };
    let mut after = None;
    let mut cursors = BTreeSet::new();
    let mut nodes = BTreeSet::new();
    for turn in 0..bounds.pages {
        let page = match page(coordinate, bounds, after.as_deref()).await {
            Ok(page) => page,
            Err(error) => {
                observed.complete = false;
                observed.diagnostics.push(error);
                break;
            }
        };
        for raw in page.nodes {
            if !nodes.insert(raw.id.clone()) {
                continue;
            }
            append(&mut observed.observations, raw, &members, &paths);
        }
        if !page.page.more {
            break;
        }
        after = page.page.end;
        if turn + 1 == bounds.pages
            || after
                .as_ref()
                .is_none_or(|cursor| !cursors.insert(cursor.clone()))
        {
            observed.complete = false;
            observed.diagnostics.push(fault(
                "Auto observations exhausted pages or received a missing/repeated cursor",
            ));
            break;
        }
    }
    observed
        .observations
        .sort_by_key(|held| (held.issue.number, held.member.number));
    observed
}

fn append(
    target: &mut Vec<Observation>,
    raw: RawIssue,
    members: &[&IssueWorktree],
    paths: &[String],
) {
    let kind = raw.issue_type.map(|kind| kind.name).unwrap_or_default();
    let Some(automation) = automation::held(&kind, &raw.body, &raw.state) else {
        return;
    };
    for member in members {
        let paths = (automation.operation.as_deref() == Some("follow"))
            .then(|| automation::overlap(&member.claims, paths));
        if paths.as_ref().is_some_and(Vec::is_empty) {
            continue;
        }
        target.push(Observation {
            issue: Coordinate {
                owner: member.issue.owner.clone(),
                repository: member.issue.repository.clone(),
                number: raw.number,
            },
            automation: automation.clone(),
            member: member.issue.clone(),
            branch: member.branch.clone(),
            exclusive: false,
            paths,
        });
    }
}

async fn page(
    coordinate: &Coordinate,
    bounds: &Bounds<'_>,
    after: Option<&str>,
) -> Result<Page, Diagnostic> {
    let mut process =
        super::super::github::transport::Request::new(bounds.command, bounds.timeout, 1024 * 1024)
            .args(["api", "graphql", "-f"])
            .arg(format!("query={QUERY}"))
            .args(["-f", &format!("owner={}", coordinate.owner)])
            .args(["-f", &format!("repo={}", coordinate.repository)])
            .args(["-F", &format!("first={}", bounds.first)]);
    if let Some(after) = after {
        process = process.args(["-f", &format!("after={after}")]);
    }
    let output = process.run().await.map_err(fault)?;
    let envelope: Envelope = serde_json::from_slice(&output.stdout).map_err(fault)?;
    if !envelope.errors.is_empty() {
        return Err(fault(
            envelope
                .errors
                .into_iter()
                .map(|error| error.message)
                .collect::<Vec<_>>()
                .join("; "),
        ));
    }
    let page = envelope
        .data
        .map(|data| data.repository.issues)
        .ok_or_else(|| fault("Auto observation has no repository connection"))?;
    let expected = format!("{}/{}", coordinate.owner, coordinate.repository);
    for raw in &page.nodes {
        validate(raw, &expected)?;
    }
    Ok(page)
}

fn fault(message: impl std::fmt::Display) -> Diagnostic {
    Diagnostic {
        code: "automation".to_string(),
        message: message.to_string(),
    }
}

fn validate(raw: &RawIssue, expected: &str) -> Result<(), Diagnostic> {
    let url = format!("https://github.com/{expected}/issues/{}", raw.number);
    if raw.repository.name_with_owner != expected || raw.url != url {
        return Err(fault("Auto observation contains a foreign Issue"));
    }
    if raw.number < 1 || raw.id.trim().is_empty() {
        return Err(fault("Auto observation has no valid Issue identity"));
    }
    if !raw.state.eq_ignore_ascii_case("open") {
        return Err(fault("Auto observation contains a non-open Issue"));
    }
    Ok(())
}
