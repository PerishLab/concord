use concord_core::Result;
use serde::Deserialize;
use std::collections::BTreeSet;

use super::super::model::{Connection, Header};
use super::{Reader, Scan, native};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub(super) struct Node {
    pub id: String,
    pub state: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::dispatch::acceptance) struct Relations {
    children: Vec<Node>,
    blockers: Vec<Node>,
}

impl Relations {
    pub fn children(&self) -> bool {
        self.children.iter().all(|node| node.state == "CLOSED")
    }

    pub fn blockers(&self) -> bool {
        self.blockers.iter().all(|node| node.state == "CLOSED")
    }
}

pub(super) struct Repository {
    pub id: String,
    pub issue: Issue,
}

pub(super) struct Issue {
    pub id: String,
    pub body: String,
    pub state: String,
    pub updated: String,
    pub children: Connection<Node>,
    pub blockers: Connection<Node>,
}

impl Reader<'_> {
    pub(in crate::dispatch::acceptance) async fn relations(
        &self,
        header: &Header,
    ) -> Result<Relations> {
        let client = self.native()?;
        let mut children = Scan::default();
        let mut blockers = Scan::default();
        for _ in 0..self.pages {
            let cursors = [&children.cursor, &blockers.cursor];
            let current = native::relationships(&client, cursors)?;
            let issue = checked(header, current)?;
            children.append(issue.children)?;
            blockers.append(issue.blockers)?;
            bounded(&children.items)?;
            bounded(&blockers.items)?;
            if children.complete && blockers.complete {
                return Ok(Relations {
                    children: children.items,
                    blockers: blockers.items,
                });
            }
        }
        Err(super::super::fault(
            "truncated",
            "native acceptance relationships exceed the selected page bound",
        ))
    }
}

fn checked(header: &Header, repository: Repository) -> Result<Issue> {
    let issue = repository.issue;
    let expected = [
        &header.repository,
        &header.node,
        &header.body,
        &header.state,
        &header.updated,
    ];
    let observed = [
        &repository.id,
        &issue.id,
        &issue.body,
        &issue.state,
        &issue.updated,
    ];
    if observed != expected {
        return Err(super::super::fault(
            "changed",
            "native relationships disagree with the observed Issue",
        ));
    }
    Ok(issue)
}

fn bounded(nodes: &[Node]) -> Result<()> {
    let bytes: usize = nodes
        .iter()
        .map(|node| node.id.len() + node.state.len())
        .sum();
    if bytes > 8 * 1024 * 1024 {
        return Err(super::super::fault(
            "bounds",
            "native acceptance relationships exceed their retained byte bound",
        ));
    }
    let mut seen = BTreeSet::new();
    for node in nodes {
        if node.id.trim().is_empty() || !seen.insert(&node.id) {
            return Err(super::super::fault(
                "identity",
                "missing or repeated native related Issue identity",
            ));
        }
        if !matches!(node.state.as_str(), "OPEN" | "CLOSED") {
            return Err(super::super::fault(
                "reply",
                "unknown related Issue lifecycle state",
            ));
        }
    }
    Ok(())
}
