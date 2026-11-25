use crate::runtime::github;
use concord_core::{Error, Repository, Result};
use runseal::github::{Number, State, lifecycle::Selection};
use std::collections::{BTreeMap, BTreeSet};

pub struct Pull {
    state: String,
    branch: String,
    head: String,
    origin: Option<String>,
    target: String,
    body: String,
}

#[derive(PartialEq, Eq)]
struct Identity<'a> {
    state: &'a str,
    branch: &'a str,
    head: &'a str,
    origin: Option<&'a str>,
    target: &'a str,
}

impl Pull {
    pub fn matches(&self, repository: &Repository, branch: &str, head: &str, issue: u64) -> bool {
        let name = format!("{}/{}", repository.owner, repository.name);
        let expected = Identity {
            state: "OPEN",
            branch,
            head,
            origin: Some(&name),
            target: &name,
        };
        let observed = Identity {
            state: &self.state,
            branch: &self.branch,
            head: &self.head,
            origin: self.origin.as_deref(),
            target: &self.target,
        };
        let reference = self
            .body
            .lines()
            .find(|line| !line.trim().is_empty())
            .map(str::trim);
        let subject = format!("{name}#{issue}");
        observed == expected
            && [format!("Refs {subject}"), format!("Closes {subject}")]
                .iter()
                .any(|line| reference == Some(line.as_str()))
    }
}

pub async fn observe(
    repository: &Repository,
    branches: &BTreeSet<String>,
    timeout: u64,
) -> Result<BTreeMap<String, Vec<Pull>>> {
    if branches.is_empty() {
        return Ok(BTreeMap::new());
    }
    let client = github::client(timeout).map_err(provider)?;
    let window = Number::new(1, 3).map_err(|error| provider(github::describe(error)))?;
    let name = format!("{}/{}", repository.owner, repository.name);
    let mut observed = BTreeMap::new();
    for branch in branches {
        let selection = Selection::new(branch, None, State::Open, window)
            .map_err(|error| provider(github::describe(error)))?;
        let slice = github::call("pull.listing", || {
            client.listing(&repository.owner, &repository.name, &selection)
        })
        .map_err(|error| provider(github::describe(error)))?
        .value;
        if slice.next.is_some() || slice.rows.len() > 2 {
            return Err(provider(format!(
                "projection query for {branch} exceeds its bound"
            )));
        }
        let pulls = slice
            .rows
            .into_iter()
            .map(|pull| Pull {
                state: pull.state.to_ascii_uppercase(),
                branch: pull.head.branch,
                head: pull.head.sha,
                origin: Some(name.clone()),
                target: name.clone(),
                body: pull.body.unwrap_or_default(),
            })
            .collect();
        observed.insert(branch.clone(), pulls);
    }
    Ok(observed)
}

fn provider(message: impl Into<String>) -> Error {
    Error::typed("concord.delivery.provider", message)
}
