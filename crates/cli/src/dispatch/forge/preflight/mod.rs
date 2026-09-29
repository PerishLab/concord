mod existing;
mod github;

pub(super) use existing::{Request as ExistingRequest, run as existing};

use super::projection::{self, Issue};
use concord_core::occupancy::Subject;
use concord_core::{Coordinate, Error, Estate, Result, activity, occupancy};
use serde::Serialize;
use std::collections::BTreeSet;
use std::path::Path;

const SCHEMA: &str = "concord.issue-preflight/v1";

pub struct Request<'a> {
    pub repository: &'a str,
    pub kind: &'a str,
    pub title: &'a str,
    pub outcome: &'a str,
    pub bounds: Bounds<'a>,
}

pub struct Bounds<'a> {
    pub command: &'a Path,
    pub first: u16,
    pub pages: u16,
    pub timeout: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Proposal {
    pub owner: String,
    pub repository: String,
    pub kind: String,
    pub title: String,
    pub outcome: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Member {
    pub name: String,
    pub branch: String,
    pub claims: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Execution {
    pub members: Vec<Member>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub activity: Option<activity::Snapshot>,
    pub occupancy: Vec<occupancy::Observation>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Candidate {
    pub issue: Issue,
    pub kind: String,
    pub updated: String,
    pub signals: Vec<String>,
    pub execution: Execution,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Diagnostic {
    pub code: String,
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Preflight {
    pub schema: &'static str,
    pub proposal: Proposal,
    pub candidates: Vec<Candidate>,
    pub diagnostics: Vec<Diagnostic>,
    pub complete: bool,
    pub observed_at: u64,
}

pub async fn run(estate: &Estate, request: Request<'_>) -> Result<Preflight> {
    let proposal = proposal(&request)?;
    let query = query(&proposal)?;
    let found = github::search(github::Request {
        bounds: &request.bounds,
        query: &query,
    })
    .await;
    let anchors = estate.issues().await?;
    let members = estate.issue_worktrees().await?;
    let occupancy = estate.occupancy()?;
    let title = terms(&proposal.title);
    let outcome = terms(&proposal.outcome);
    let mut candidates = Vec::new();
    for raw in found.issues {
        if let Some(candidate) = candidate(&proposal, &title, &outcome, raw) {
            let anchor = anchors
                .iter()
                .find(|anchor| anchor.node == candidate.issue.node);
            let held = members
                .iter()
                .filter(|member| member.node == candidate.issue.node)
                .map(|member| Member {
                    name: member.name.clone(),
                    branch: member.branch.clone(),
                    claims: member.claims.clone(),
                })
                .collect();
            let activity = anchor.map(|anchor| estate.activity(anchor)).transpose()?;
            let occupied = occupancy
                .observations
                .iter()
                .filter(|held| matches(held, &candidate.issue.node))
                .cloned()
                .collect();
            candidates.push(Candidate {
                execution: Execution {
                    members: held,
                    activity,
                    occupancy: occupied,
                },
                ..candidate
            });
        }
    }
    candidates.sort_by_key(|candidate| candidate.issue.coordinate.number);
    Ok(Preflight {
        schema: SCHEMA,
        proposal,
        candidates,
        diagnostics: found.diagnostics,
        complete: found.complete,
        observed_at: projection::now(),
    })
}

fn proposal(request: &Request<'_>) -> Result<Proposal> {
    let (owner, repository) = request.repository.split_once('/').ok_or_else(|| {
        Error::typed(
            "concord.issue.preflight.repository",
            "repository must be OWNER/REPOSITORY",
        )
    })?;
    filled("owner", owner)?;
    filled("repository", repository)?;
    if repository.contains('/') {
        return Err(Error::typed(
            "concord.issue.preflight.repository",
            "repository must be OWNER/REPOSITORY",
        ));
    }
    filled("kind", request.kind)?;
    filled("title", request.title)?;
    filled("outcome", request.outcome)?;
    Ok(Proposal {
        owner: owner.to_string(),
        repository: repository.to_string(),
        kind: request.kind.trim().to_string(),
        title: request.title.trim().to_string(),
        outcome: request.outcome.trim().to_string(),
    })
}

fn filled(field: &str, value: &str) -> Result<()> {
    if value.trim().is_empty() {
        return Err(Error::typed(
            "concord.issue.preflight.proposal",
            format!("proposed Issue {field} must be non-empty"),
        ));
    }
    Ok(())
}

fn candidate(
    proposal: &Proposal,
    title: &BTreeSet<String>,
    outcome: &BTreeSet<String>,
    raw: github::RawIssue,
) -> Option<Candidate> {
    let expected = format!("{}/{}", proposal.owner, proposal.repository);
    if raw.repository.name_with_owner != expected || !raw.state.eq_ignore_ascii_case("open") {
        return None;
    }
    let words = terms(&format!("{} {}", raw.title, raw.body));
    let mut signals = Vec::new();
    if raw.title.trim().eq_ignore_ascii_case(proposal.title.trim()) {
        signals.push("exact-title".to_string());
    }
    if intersects(title, &words) {
        signals.push("title-terms".to_string());
    }
    if intersects(outcome, &words) {
        signals.push("outcome-terms".to_string());
    }
    if signals.is_empty() {
        return None;
    }
    Some(Candidate {
        issue: Issue {
            node: raw.id,
            coordinate: Coordinate {
                owner: proposal.owner.clone(),
                repository: proposal.repository.clone(),
                number: raw.number,
            },
            url: raw.url,
            title: raw.title,
            state: raw.state.to_ascii_lowercase(),
        },
        kind: raw.issue_type.map(|kind| kind.name).unwrap_or_default(),
        updated: raw.updated_at,
        signals,
        execution: Execution {
            members: Vec::new(),
            activity: None,
            occupancy: Vec::new(),
        },
    })
}

fn matches(observation: &occupancy::Observation, expected: &str) -> bool {
    observation
        .holder
        .subjects
        .iter()
        .any(|subject| match subject {
            Subject::Issue { node } | Subject::IssueMember { node, .. } => node == expected,
            Subject::Surface { .. } => false,
        })
}

fn query(proposal: &Proposal) -> Result<String> {
    let mut held = terms(&format!("{} {}", proposal.title, proposal.outcome))
        .into_iter()
        .collect::<Vec<_>>();
    if held.is_empty() {
        return Err(Error::typed(
            "concord.issue.preflight.terms",
            "title and outcome contain no searchable terms",
        ));
    }
    held.sort_by(|left, right| {
        right
            .chars()
            .count()
            .cmp(&left.chars().count())
            .then_with(|| left.cmp(right))
    });
    held.truncate(5);
    let held = held
        .into_iter()
        .map(|term| format!("\"{}\"", term.replace('"', "")))
        .collect::<Vec<_>>()
        .join(" OR ");
    Ok(format!(
        "repo:{}/{} is:issue is:open in:title,body {held}",
        proposal.owner, proposal.repository
    ))
}

fn terms(value: &str) -> BTreeSet<String> {
    value
        .split(|character: char| !character.is_alphanumeric())
        .map(str::trim)
        .filter(|term| term.chars().count() >= 3)
        .map(str::to_lowercase)
        .filter(|term| {
            !matches!(
                term.as_str(),
                "and" | "are" | "before" | "for" | "from" | "into" | "the" | "this" | "with"
            )
        })
        .collect()
}

fn intersects(left: &BTreeSet<String>, right: &BTreeSet<String>) -> bool {
    left.iter().any(|term| right.contains(term))
}
