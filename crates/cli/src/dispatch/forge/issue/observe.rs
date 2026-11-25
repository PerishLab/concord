use crate::runtime::github;
use concord_core::{Coordinate, Error, Result};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Observed {
    pub node: String,
    pub stable: String,
    #[serde(flatten)]
    pub coordinate: Coordinate,
    pub url: String,
    pub state: String,
    pub kind: String,
    pub updated: String,
    pub seen: u64,
}

struct Reply {
    node: String,
    stable: String,
    number: i64,
    url: String,
    state: String,
    kind: String,
    updated: String,
}

pub async fn issue(coordinate: &Coordinate, timeout: u64) -> Result<Observed> {
    let client = github::client(timeout).map_err(|error| fault("provider", error))?;
    let number = u32::try_from(coordinate.number)
        .map_err(|_| fault("coordinate", "invalid Issue number"))?;
    let target = runseal::github::Target::new(&coordinate.owner, &coordinate.repository, number)
        .map_err(|error| fault("coordinate", error.to_string()))?;
    let issue = github::call("issue.show", || client.issue(&target))
        .map_err(|error| fault("provider", github::describe(error)))?
        .value;
    shape(
        coordinate,
        Reply {
            node: issue.id,
            stable: issue.repository.id,
            number: issue.number.into(),
            url: issue.url,
            state: issue.state,
            kind: issue.kind.map(|kind| kind.name).unwrap_or_default(),
            updated: issue.updated,
        },
    )
}

fn shape(coordinate: &Coordinate, reply: Reply) -> Result<Observed> {
    let expected = format!(
        "https://github.com/{}/{}/issues/{}",
        coordinate.owner, coordinate.repository, coordinate.number
    );
    if reply.number != coordinate.number || reply.url != expected {
        return Err(fault(
            "coordinate",
            "GitHub Issue reply does not match the requested coordinate",
        ));
    }
    if reply.node.trim().is_empty() {
        return Err(fault("node", "GitHub Issue has no stable node identity"));
    }
    if reply.stable.trim().is_empty() {
        return Err(fault(
            "repository",
            "GitHub repository has no stable node identity",
        ));
    }
    if reply.kind.trim().is_empty() {
        return Err(fault("type", "GitHub Issue has no enabled native type"));
    }
    let state = reply.state.to_ascii_lowercase();
    if !matches!(state.as_str(), "open" | "closed") || reply.updated.is_empty() {
        return Err(fault(
            "malformed",
            "GitHub Issue reply has an invalid state or update time",
        ));
    }
    Ok(Observed {
        node: reply.node,
        stable: reply.stable,
        coordinate: coordinate.clone(),
        url: reply.url,
        state,
        kind: reply.kind,
        updated: reply.updated,
        seen: now(),
    })
}

fn fault(kind: &str, message: impl Into<String>) -> Error {
    Error::typed(format!("concord.issue.observe.{kind}"), message.into())
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
