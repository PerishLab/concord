use crate::args::acceptance::Closing;
use crate::dispatch::acceptance::model::Connection;
use crate::dispatch::acceptance::read::Scan;
use crate::runtime::github;
use concord_core::{Coordinate, Result};
use runseal::github::{Target, Window};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

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
        let client =
            github::client(self.args.observe.timeout).map_err(|error| fault("provider", error))?;
        let number = u32::try_from(self.coordinate.number)
            .map_err(|_| fault("identity", "invalid PR number"))?;
        let target = Target::new(&self.coordinate.owner, &self.coordinate.repository, number)
            .map_err(|error| fault("identity", github::describe(error)))?;
        let window = Window::new(100, cursor.as_deref())
            .map_err(|error| fault("provider", github::describe(error)))?;
        let found = github::call("pull.closing", || client.closing(&target, &window))
            .map_err(|error| fault("provider", github::describe(error)))?
            .value;
        let pull: Pull = serde_json::from_value(found.pull)
            .map_err(|error| fault("reply", error.to_string()))?;
        Ok(Repository {
            id: found.repository,
            pull,
        })
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
