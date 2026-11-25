use super::github::{RawIssue, RawRepository, RawType};
use super::{Bounds, Diagnostic};
use crate::runtime::github;
use concord_core::{Coordinate, Estate, IssueWorktree, automation};
use runseal::github::records;
use serde::Serialize;
use std::collections::BTreeSet;
use std::path::Path;

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

pub(super) async fn observe(
    estate: &Estate,
    coordinate: &Coordinate,
    bounds: &Bounds,
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
    let mut page = 1u32;
    let mut seen = BTreeSet::new();
    let mut nodes = BTreeSet::new();
    for turn in 0..bounds.pages {
        let listing = match slice(coordinate, bounds, page) {
            Ok(listing) => listing,
            Err(error) => {
                observed.complete = false;
                observed.diagnostics.push(error);
                break;
            }
        };
        for raw in listing.0 {
            if !nodes.insert(raw.id.clone()) {
                continue;
            }
            append(&mut observed.observations, raw, &members, &paths);
        }
        let Some(next) = listing.1 else {
            break;
        };
        if turn + 1 == bounds.pages || !seen.insert(next) {
            observed.complete = false;
            observed.diagnostics.push(fault(
                "Auto observations exhausted pages or received a missing/repeated cursor",
            ));
            break;
        }
        page = next;
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

fn slice(
    coordinate: &Coordinate,
    bounds: &Bounds,
    page: u32,
) -> Result<(Vec<RawIssue>, Option<u32>), Diagnostic> {
    let client = github::client(bounds.timeout).map_err(fault)?;
    let size = u8::try_from(bounds.first).map_err(|_| fault("invalid page size"))?;
    let listing = github::call("issue.list", || {
        client.issues(&coordinate.owner, &coordinate.repository, page, size)
    })
    .map_err(|error| fault(github::describe(error)))?
    .value;
    let expected = format!("{}/{}", coordinate.owner, coordinate.repository);
    let mut rows = Vec::with_capacity(listing.rows.len());
    for record in listing.rows {
        let raw = convert(record, &listing.repository);
        validate(&raw, &expected)?;
        rows.push(raw);
    }
    Ok((rows, listing.next))
}

fn convert(record: records::Record, repository: &str) -> RawIssue {
    let state = match record.state {
        records::State::Open => "OPEN",
        records::State::Closed => "CLOSED",
    };
    RawIssue {
        id: record.node,
        number: i64::from(record.number),
        url: record.url,
        title: record.title,
        body: record.body.unwrap_or_default(),
        state: state.into(),
        updated_at: record.updated,
        issue_type: record.kind.map(|kind| RawType { name: kind.name }),
        repository: RawRepository {
            name_with_owner: repository.into(),
        },
    }
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
