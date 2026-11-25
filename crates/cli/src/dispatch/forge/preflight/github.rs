use super::Diagnostic;
use crate::runtime::github;
use runseal::github::{Outcome, Window};
use std::collections::BTreeMap;

pub(super) struct Request<'a> {
    pub bounds: &'a super::Bounds,
    pub query: &'a str,
}

pub(super) struct Search {
    pub issues: Vec<RawIssue>,
    pub diagnostics: Vec<Diagnostic>,
    pub complete: bool,
}

#[derive(Clone)]
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

#[derive(Clone)]
pub(super) struct RawType {
    pub name: String,
}

#[derive(Clone)]
pub(super) struct RawRepository {
    pub name_with_owner: String,
}

pub(super) async fn search(request: Request<'_>) -> Search {
    let mut issues = BTreeMap::new();
    let mut diagnostics = Vec::new();
    let mut cursor = None;
    let mut complete = true;
    for turn in 0..request.bounds.pages {
        let found = match page(&request, cursor.as_deref()) {
            Ok(found) => found,
            Err(fault) => {
                complete = false;
                diagnostics.push(fault);
                break;
            }
        };
        let more = found.next.is_some();
        cursor = found.next;
        for entry in found.rows {
            issues.insert(entry.id.clone(), convert(entry));
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

fn page(
    request: &Request<'_>,
    after: Option<&str>,
) -> Result<runseal::github::search::Search, Diagnostic> {
    let client =
        github::client(request.bounds.timeout).map_err(|error| fault("provider", error))?;
    let size = u8::try_from(request.bounds.first)
        .map_err(|_| fault("provider", "invalid search page size"))?;
    let window = Window::new(size, after).map_err(translate)?;
    github::call("issue.search", || client.search(request.query, &window))
        .map(|receipt| receipt.value)
        .map_err(translate)
}

fn convert(entry: runseal::github::search::Entry) -> RawIssue {
    RawIssue {
        id: entry.id,
        number: i64::from(entry.number),
        url: entry.url,
        title: entry.title,
        body: entry.body,
        state: entry.state,
        updated_at: entry.updated,
        issue_type: entry.kind.map(|kind| RawType { name: kind.name }),
        repository: RawRepository {
            name_with_owner: entry.repository.name,
        },
    }
}

fn translate(error: runseal::github::Error) -> Diagnostic {
    let code = match error.outcome {
        Outcome::Unsent => "command",
        _ => "provider",
    };
    fault(code, github::describe(error))
}

fn fault(code: &str, message: impl Into<String>) -> Diagnostic {
    Diagnostic {
        code: code.to_string(),
        message: message.into(),
    }
}
