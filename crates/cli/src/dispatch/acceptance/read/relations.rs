use concord_core::Result;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeSet;

use super::super::super::forge::Request;
use super::super::model::{Connection, Header};
use super::{Reader, Scan};

const QUERY: &str = "query($owner:String!,$name:String!,$number:Int!,$children:String,$blockers:String){repository(owner:$owner,name:$name){id issue(number:$number){id body state updated:updatedAt children:subIssues(first:100,after:$children){total:totalCount nodes{id state} page:pageInfo{next:hasNextPage cursor:endCursor}} blockers:blockedBy(first:100,after:$blockers){total:totalCount nodes{id state} page:pageInfo{next:hasNextPage cursor:endCursor}}}}}";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
struct Node {
    id: String,
    state: String,
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

#[derive(Deserialize)]
struct Envelope {
    data: Data,
    errors: Option<Value>,
}

#[derive(Deserialize)]
struct Data {
    repository: Repository,
}

#[derive(Deserialize)]
struct Repository {
    id: String,
    issue: Issue,
}

#[derive(Deserialize)]
struct Issue {
    id: String,
    body: String,
    state: String,
    updated: String,
    children: Connection<Node>,
    blockers: Connection<Node>,
}

impl Reader<'_> {
    pub(in crate::dispatch::acceptance) async fn relations(
        &self,
        header: &Header,
    ) -> Result<Relations> {
        let mut children = Scan::default();
        let mut blockers = Scan::default();
        for _ in 0..self.pages {
            let issue = self
                .relationships(header, [&children.cursor, &blockers.cursor])
                .await?;
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

    async fn relationships(&self, header: &Header, cursors: [&Option<String>; 2]) -> Result<Issue> {
        let input = serde_json::to_vec(&json!({"query": QUERY, "variables": {
            "owner": self.coordinate.owner, "name": self.coordinate.repository, "number": self.coordinate.number,
            "children": cursors[0], "blockers": cursors[1],
        }})).map_err(|error| super::super::fault("encode", error.to_string()))?;
        let reply = Request::new(self.command, self.timeout, 8 * 1024 * 1024)
            .args(["api", "graphql", "--input", "-"])
            .input(&input)
            .run()
            .await
            .map_err(|error| super::super::fault("provider", error.to_string()))?;
        let envelope: Envelope = serde_json::from_slice(&reply.stdout)
            .map_err(|error| super::super::fault("reply", error.to_string()))?;
        if envelope.errors.is_some() {
            return Err(super::super::fault(
                "provider",
                "relationship observation contains provider errors",
            ));
        }
        let repository = envelope.data.repository;
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
