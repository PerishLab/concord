use concord_core::{Coordinate, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeSet;

use crate::args::acceptance::Closing;
use crate::dispatch::acceptance::model::Connection;
use crate::dispatch::acceptance::read::Scan;
use crate::dispatch::forge::Request;

const QUERY: &str = "query($owner:String!,$name:String!,$number:Int!,$after:String){repository(owner:$owner,name:$name){id pull:pullRequest(number:$number){id number url head:headRefOid body state updated:updatedAt references:closingIssuesReferences(first:100,after:$after){total:totalCount nodes{id number url repository{name owner{login}}} page:pageInfo{next:hasNextPage cursor:endCursor}}}}}";

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(super) struct Snapshot {
    pub node: String,
    pub repository: String,
    pub coordinate: Coordinate,
    pub head: String,
    pub body: String,
    pub state: String,
    pub updated: String,
    pub references: Vec<Link>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(super) struct Link {
    pub node: String,
    pub coordinate: Coordinate,
    pub url: String,
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
    pull: Pull,
}

#[derive(Deserialize)]
struct Pull {
    id: String,
    number: i64,
    url: String,
    head: String,
    body: String,
    state: String,
    updated: String,
    references: Connection<Issue>,
}

#[derive(Deserialize)]
struct Issue {
    id: String,
    number: i64,
    url: String,
    repository: Name,
}

#[derive(Deserialize)]
struct Name {
    name: String,
    owner: Owner,
}

#[derive(Deserialize)]
struct Owner {
    login: String,
}

pub(super) struct Reader<'a> {
    pub args: &'a Closing,
    pub coordinate: &'a Coordinate,
}

impl Reader<'_> {
    pub async fn complete(&self) -> Result<Snapshot> {
        let mut held = None;
        let mut links = Scan::default();
        for _ in 0..self.args.observe.pages {
            let repository = self.page(&links.cursor).await?;
            let pull = repository.pull;
            let current = self.snapshot(repository.id, &pull)?;
            if held.as_ref().is_some_and(|previous| previous != &current) {
                return Err(fault(
                    "changed",
                    "PR content or head changed between closing-reference pages",
                ));
            }
            held = Some(current);
            links.append(pull.references)?;
            let bytes: usize = links
                .items
                .iter()
                .map(|issue| {
                    issue.id.len()
                        + issue.url.len()
                        + issue.repository.name.len()
                        + issue.repository.owner.login.len()
                })
                .sum();
            if bytes > 8 * 1024 * 1024 {
                return Err(fault(
                    "bounds",
                    "retained native closing references exceed their byte bound",
                ));
            }
            if links.complete {
                let mut snapshot = held.unwrap();
                snapshot.references = references(links.items)?;
                return Ok(snapshot);
            }
        }
        Err(fault(
            "truncated",
            "PR closing references exceed the selected page bound",
        ))
    }

    async fn page(&self, cursor: &Option<String>) -> Result<Repository> {
        let input = serde_json::to_vec(&json!({"query": QUERY, "variables": {
            "owner": self.coordinate.owner, "name": self.coordinate.repository,
            "number": self.coordinate.number, "after": cursor,
        }}))
        .map_err(|error| fault("encode", error.to_string()))?;
        let reply = Request::new(
            &self.args.observe.command,
            self.args.observe.timeout,
            8 * 1024 * 1024,
        )
        .args(["api", "graphql", "--input", "-"])
        .input(&input)
        .run()
        .await
        .map_err(|error| fault("provider", error.to_string()))?;
        let envelope: Envelope = serde_json::from_slice(&reply.stdout)
            .map_err(|error| fault("reply", error.to_string()))?;
        if envelope.errors.is_some() {
            return Err(fault(
                "provider",
                "PR closing-reference observation contains provider errors",
            ));
        }
        Ok(envelope.data.repository)
    }

    fn snapshot(&self, repository: String, pull: &Pull) -> Result<Snapshot> {
        let url = format!(
            "https://github.com/{}/{}/pull/{}",
            self.coordinate.owner, self.coordinate.repository, self.coordinate.number
        );
        if pull.number != self.coordinate.number || pull.url != url {
            return Err(fault(
                "identity",
                "PR observation disagrees with its exact coordinate",
            ));
        }
        if pull.head != self.args.head {
            return Err(fault(
                "head",
                "current PR head differs from the explicit CI head",
            ));
        }
        if !matches!(pull.state.as_str(), "OPEN" | "CLOSED" | "MERGED") || pull.body.len() > 65_536
        {
            return Err(fault("reply", "unknown PR state or oversized PR body"));
        }
        for value in [&repository, &pull.id, &pull.updated] {
            if value.trim().is_empty() {
                return Err(fault("identity", "missing current PR identity metadata"));
            }
        }
        Ok(Snapshot {
            node: pull.id.clone(),
            repository,
            coordinate: self.coordinate.clone(),
            head: pull.head.clone(),
            body: pull.body.clone(),
            state: pull.state.clone(),
            updated: pull.updated.clone(),
            references: Vec::new(),
        })
    }
}

fn references(issues: Vec<Issue>) -> Result<Vec<Link>> {
    let mut links = Vec::new();
    let mut nodes = BTreeSet::new();
    let mut urls = BTreeSet::new();
    let mut bytes = 0;
    for issue in issues {
        let coordinate = Coordinate::parse(&format!(
            "{}/{}#{}",
            issue.repository.owner.login, issue.repository.name, issue.number
        ))?;
        let url = format!(
            "https://github.com/{}/{}/issues/{}",
            coordinate.owner, coordinate.repository, coordinate.number
        );
        bytes += issue.id.len() + url.len();
        if bytes > 8 * 1024 * 1024 {
            return Err(fault(
                "bounds",
                "PR closing references exceed their retained byte bound",
            ));
        }
        if issue.id.trim().is_empty()
            || !nodes.insert(issue.id.clone())
            || !urls.insert(url.clone())
        {
            return Err(fault(
                "identity",
                "missing or repeated closing Issue identity",
            ));
        }
        if issue.url != url {
            return Err(fault(
                "identity",
                "native closing Issue URL disagrees with its coordinate",
            ));
        }
        links.push(Link {
            node: issue.id,
            coordinate,
            url,
        });
    }
    Ok(links)
}

fn fault(kind: &str, message: impl Into<String>) -> concord_core::Error {
    crate::dispatch::acceptance::fault(kind, message)
}
