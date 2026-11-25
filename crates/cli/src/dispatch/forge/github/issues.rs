use crate::runtime::github;
use concord_core::{Error, Repository, Result};
use runseal::github::classify::Status;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Eq, PartialEq)]
pub enum Observed {
    Absent,
    Issue { open: bool, kind: Option<String> },
}

pub async fn states(
    repository: &Repository,
    numbers: &BTreeSet<u64>,
    timeout: u64,
) -> Result<BTreeMap<u64, Observed>> {
    if numbers.is_empty() {
        return Ok(BTreeMap::new());
    }
    let client = github::client(timeout).map_err(provider)?;
    let requested = numbers
        .iter()
        .map(|&number| u32::try_from(number).map_err(|_| provider("invalid Issue number")))
        .collect::<Result<Vec<_>>>()?;
    let observed = github::call("issue.classify", || {
        client.classify(&repository.owner, &repository.name, &requested)
    })
    .map_err(|error| provider(github::describe(error)))?
    .value;
    Ok(observed
        .into_iter()
        .map(|(number, status)| {
            let held = match status {
                Status::Absent => Observed::Absent,
                Status::Issue { open, kind } => Observed::Issue { open, kind },
            };
            (u64::from(number), held)
        })
        .collect())
}

fn provider(message: impl Into<String>) -> Error {
    Error::typed("concord.delivery.provider", message)
}
