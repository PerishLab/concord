use super::super::projection::{self, Connection, Issue, Pull};
use super::{Diagnostic, Execution, Member, matches};
use concord_core::{Anchor, Coordinate, Estate, IssueWorktree, Result};
use serde::Serialize;
use std::collections::BTreeSet;
use std::path::Path;

const SCHEMA: &str = "concord.issue-preflight-existing/v1";

pub struct Request<'a> {
    pub issue: &'a str,
    pub command: &'a Path,
    pub page_size: u16,
    pub max_pages: u16,
    pub timeout: u64,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Relation {
    Parent,
    #[serde(rename = "sub-issue")]
    Child,
    BlockedBy,
    Blocking,
    #[serde(rename = "linked-pull")]
    Pull,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Overlap {
    pub root: String,
    pub related: String,
    pub paths: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Relationship {
    pub kind: Relation,
    pub issue: Issue,
    pub execution: Execution,
    pub overlaps: Vec<Overlap>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PullRelation {
    pub kind: Relation,
    pub pull: Pull,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Existing {
    pub schema: &'static str,
    pub issue: Issue,
    pub kind: String,
    pub execution: Execution,
    pub relationships: Vec<Relationship>,
    pub pulls: Vec<PullRelation>,
    pub diagnostics: Vec<Diagnostic>,
    pub complete: bool,
    pub observed_at: u64,
}

pub async fn run(estate: &Estate, request: Request<'_>) -> Result<Existing> {
    let coordinate = Coordinate::parse(request.issue)?;
    let mut connections = BTreeSet::new();
    connections.extend([
        Connection::SubIssues,
        Connection::BlockedBy,
        Connection::Blocking,
        Connection::Pulls,
    ]);
    let projected = projection::Projection::new(
        request.command,
        request.page_size,
        request.max_pages,
        request.timeout,
    )
    .complete(&coordinate, connections)
    .await;
    let complete = match projected {
        Ok(complete) => complete,
        Err(fault) => {
            return Ok(Existing {
                schema: SCHEMA,
                issue: placeholder(&coordinate),
                kind: String::new(),
                execution: empty(),
                relationships: Vec::new(),
                pulls: Vec::new(),
                diagnostics: vec![Diagnostic {
                    code: format!("concord.issue.projection.{}", fault.code),
                    message: fault.message,
                }],
                complete: false,
                observed_at: projection::now(),
            });
        }
    };
    let anchors = estate.issues().await?;
    let members = estate.issue_worktrees().await?;
    let occupancy = estate.occupancy()?;
    let root = held(&members, &complete.issue.node);
    let view = View {
        estate,
        anchors: &anchors,
        members: &members,
        occupancy: &occupancy,
        root: &root,
    };
    let execution = execution(&view, &root, &complete.issue.node)?;
    let mut relationships = Vec::new();
    add(&mut relationships, complete.parent, Relation::Parent, &view)?;
    add(
        &mut relationships,
        complete.sub_issues,
        Relation::Child,
        &view,
    )?;
    add(
        &mut relationships,
        complete.blocked_by,
        Relation::BlockedBy,
        &view,
    )?;
    add(
        &mut relationships,
        complete.blocking,
        Relation::Blocking,
        &view,
    )?;
    relationships.sort_by_key(|held| (held.kind.clone(), held.issue.coordinate.number));
    Ok(Existing {
        schema: SCHEMA,
        issue: complete.issue,
        kind: complete.kind,
        execution,
        relationships,
        pulls: complete
            .pulls
            .into_iter()
            .map(|pull| PullRelation {
                kind: Relation::Pull,
                pull,
            })
            .collect(),
        diagnostics: Vec::new(),
        complete: true,
        observed_at: projection::now(),
    })
}

struct View<'a> {
    estate: &'a Estate,
    anchors: &'a [Anchor],
    members: &'a [IssueWorktree],
    occupancy: &'a concord_core::occupancy::Snapshot,
    root: &'a [&'a IssueWorktree],
}

fn add(
    target: &mut Vec<Relationship>,
    issues: impl IntoIterator<Item = Issue>,
    kind: Relation,
    view: &View<'_>,
) -> Result<()> {
    for issue in issues {
        let related = held(view.members, &issue.node);
        target.push(Relationship {
            execution: execution(view, &related, &issue.node)?,
            overlaps: intersections(view.estate, view.root, &related)?,
            kind: kind.clone(),
            issue,
        });
    }
    Ok(())
}

fn execution(view: &View<'_>, members: &[&IssueWorktree], node: &str) -> Result<Execution> {
    let activity = view
        .anchors
        .iter()
        .find(|anchor| anchor.node == node)
        .map(|anchor| view.estate.activity(anchor))
        .transpose()?;
    Ok(Execution {
        members: members
            .iter()
            .map(|member| Member {
                name: member.name.clone(),
                branch: member.branch.clone(),
                claims: member.claims.clone(),
            })
            .collect(),
        activity,
        occupancy: view
            .occupancy
            .observations
            .iter()
            .filter(|observation| matches(observation, node))
            .cloned()
            .collect(),
    })
}

fn intersections(
    estate: &Estate,
    root: &[&IssueWorktree],
    related: &[&IssueWorktree],
) -> Result<Vec<Overlap>> {
    let mut found = Vec::new();
    for left in root {
        for right in related {
            let paths = estate.claim_intersections(left, right)?;
            if !paths.is_empty() {
                found.push(Overlap {
                    root: left.name.clone(),
                    related: right.name.clone(),
                    paths,
                });
            }
        }
    }
    Ok(found)
}

fn held<'a>(members: &'a [IssueWorktree], node: &str) -> Vec<&'a IssueWorktree> {
    members
        .iter()
        .filter(|member| member.node == node)
        .collect()
}

fn empty() -> Execution {
    Execution {
        members: Vec::new(),
        activity: None,
        occupancy: Vec::new(),
    }
}

fn placeholder(coordinate: &Coordinate) -> Issue {
    Issue {
        node: String::new(),
        coordinate: coordinate.clone(),
        url: String::new(),
        title: String::new(),
        state: String::new(),
    }
}
