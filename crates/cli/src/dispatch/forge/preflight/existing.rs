use super::super::projection::{self, Connection, Issue, Pull};
use super::{Diagnostic, Execution, Member, matches};
use crate::args::issue::Relations;
use concord_core::{Anchor, Coordinate, Estate, Result, Worktree};
use serde::Serialize;
use std::collections::BTreeSet;

const SCHEMA: &str = "concord.issue-preflight-existing/v2";

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Relation {
    Parent,
    #[serde(rename = "sub-issue")]
    Child,
    #[serde(rename = "blocked-by")]
    Blocker,
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
pub struct Linked {
    pub kind: Relation,
    pub pull: Pull,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Existing {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub observations: Vec<super::automation::Observation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub automation: Option<concord_core::automation::Held>,
    pub schema: &'static str,
    pub issue: Issue,
    pub kind: String,
    pub execution: Execution,
    pub relationships: Vec<Relationship>,
    pub pulls: Vec<Linked>,
    pub diagnostics: Vec<Diagnostic>,
    pub complete: bool,
    #[serde(rename = "observed_at")]
    pub observed: u64,
}

pub async fn run(estate: &Estate, request: &Relations) -> Result<Existing> {
    let coordinate = Coordinate::parse(&request.issue)?;
    let mut connections = BTreeSet::new();
    connections.extend([
        Connection::Children,
        Connection::Blockers,
        Connection::Blocking,
        Connection::Pulls,
    ]);
    let projected = projection::Projection::new(request.size, request.pages, request.timeout)
        .complete(&coordinate, connections)
        .await;
    let complete = match projected {
        Ok(complete) => complete,
        Err(fault) => {
            return Ok(Existing {
                observations: Vec::new(),
                schema: SCHEMA,
                automation: None,
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
                observed: projection::now(),
            });
        }
    };
    let anchors = estate.issues().await?;
    let members = estate.worktrees().await?;
    let observed = super::automation::observe(
        estate,
        &coordinate,
        &super::Bounds {
            first: request.size,
            pages: request.pages,
            timeout: request.timeout,
        },
        &members,
    )
    .await;
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
        complete.children,
        Relation::Child,
        &view,
    )?;
    add(
        &mut relationships,
        complete.blockers,
        Relation::Blocker,
        &view,
    )?;
    add(
        &mut relationships,
        complete.blocking,
        Relation::Blocking,
        &view,
    )?;
    relationships.sort_by_key(|held| (held.kind.clone(), held.issue.coordinate.number));
    let automation =
        concord_core::automation::held(&complete.kind, &complete.body, &complete.issue.state);
    let mut diagnostics = observed.diagnostics;
    if let Some(message) = super::super::readiness::advice(&complete.body, &complete.kind) {
        diagnostics.push(Diagnostic {
            code: "concord.issue.sections".into(),
            message,
        });
    }
    Ok(Existing {
        observations: observed.observations,
        automation,
        schema: SCHEMA,
        issue: complete.issue,
        kind: complete.kind,
        execution,
        relationships,
        pulls: complete
            .pulls
            .into_iter()
            .map(|pull| Linked {
                kind: Relation::Pull,
                pull,
            })
            .collect(),
        diagnostics,
        complete: observed.complete,
        observed: projection::now(),
    })
}

struct View<'a> {
    estate: &'a Estate,
    anchors: &'a [Anchor],
    members: &'a [Worktree],
    occupancy: &'a concord_core::occupancy::Snapshot,
    root: &'a [&'a Worktree],
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

fn execution(view: &View<'_>, members: &[&Worktree], node: &str) -> Result<Execution> {
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
    root: &[&Worktree],
    related: &[&Worktree],
) -> Result<Vec<Overlap>> {
    let mut found = Vec::new();
    for left in root {
        for right in related {
            let paths = estate.intersections(left, right)?;
            if !paths.is_empty() {
                found.push(Overlap {
                    root: left.issue.identity(),
                    related: right.issue.identity(),
                    paths,
                });
            }
        }
    }
    Ok(found)
}

fn held<'a>(members: &'a [Worktree], node: &str) -> Vec<&'a Worktree> {
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
        automation: None,
        node: String::new(),
        coordinate: coordinate.clone(),
        url: String::new(),
        title: String::new(),
        state: String::new(),
    }
}
