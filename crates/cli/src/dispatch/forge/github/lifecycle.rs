use crate::args::close::Observe;
use crate::dispatch::forge::close::model::{Issue, Reason, State};
use crate::runtime::github;
use concord_core::{Coordinate, Error, Result};
use serde_json::{Value, json};

struct Repository {
    id: String,
    name: String,
    issue: Wire,
}

struct Wire {
    id: String,
    number: i64,
    url: String,
    body: String,
    state: String,
    reason: Value,
    updated: String,
    writable: bool,
    kind: Option<String>,
}

#[derive(Eq, PartialEq)]
pub(in crate::dispatch::forge) struct Observed {
    pub issue: Issue,
    pub body: String,
}

pub(in crate::dispatch::forge) async fn read(
    args: &Observe,
    coordinate: &Coordinate,
) -> Result<Observed> {
    let repository = native(coordinate, args.timeout)?;
    let raw = repository.issue;
    let url = format!(
        "https://github.com/{}/{}/issues/{}",
        coordinate.owner, coordinate.repository, coordinate.number
    );
    let identity = [
        !raw.id.is_empty(),
        !repository.id.is_empty(),
        raw.number == coordinate.number,
        raw.url == url,
        repository.name == format!("{}/{}", coordinate.owner, coordinate.repository),
        !raw.updated.is_empty(),
        matches!(raw.state.as_str(), "OPEN" | "CLOSED"),
    ];
    if !identity.into_iter().all(|valid| valid) {
        return Err(fault(
            "identity",
            "GitHub closure facts are missing, foreign or inconsistent",
        ));
    }
    let (state, reason) = match (raw.state.as_str(), raw.reason) {
        ("OPEN", Value::Null) => (State::Open, None),
        ("OPEN", Value::String(reason)) if reason == "REOPENED" => {
            (State::Open, Some(Reason::Reopened))
        }
        ("CLOSED", Value::String(reason)) if reason == "COMPLETED" => {
            (State::Closed, Some(Reason::Completed))
        }
        ("CLOSED", Value::String(reason)) if reason == "NOT_PLANNED" => {
            (State::Closed, Some(Reason::Declined))
        }
        ("CLOSED", Value::String(reason)) if reason == "DUPLICATE" => {
            (State::Closed, Some(Reason::Duplicate))
        }
        _ => {
            return Err(fault(
                "schema",
                "native lifecycle reason is unavailable or unknown",
            ));
        }
    };
    Ok(Observed {
        issue: Issue {
            node: raw.id,
            repository: repository.id,
            url: raw.url,
            kind: raw.kind.unwrap_or_default(),
            state,
            reason,
            review: concord_core::acceptance::digest(&raw.body),
            updated: raw.updated,
            writable: raw.writable,
        },
        body: raw.body,
    })
}

fn native(coordinate: &Coordinate, timeout: u64) -> Result<Repository> {
    let number =
        u32::try_from(coordinate.number).map_err(|_| fault("identity", "invalid Issue number"))?;
    let target = runseal::github::Target::new(&coordinate.owner, &coordinate.repository, number)
        .map_err(provider)?;
    let client = github::client(timeout).map_err(|error| fault("provider", error))?;
    let raw = github::call("issue.show", || client.issue(&target))
        .map_err(provider)?
        .value;
    let writable = raw
        .writable
        .ok_or_else(|| fault("identity", "Issue write capability is unavailable"))?;
    Ok(Repository {
        id: raw.repository.id,
        name: format!("{}/{}", coordinate.owner, coordinate.repository),
        issue: Wire {
            id: raw.id,
            number: raw.number.into(),
            url: raw.url,
            body: raw.body,
            state: raw.state,
            reason: json!(raw.reason),
            updated: raw.updated,
            writable,
            kind: raw.kind.map(|runseal::github::Label { name }| name),
        },
    })
}

pub(in crate::dispatch::forge) async fn close(args: &Observe, issue: &Issue) -> Result<()> {
    let client = github::client(args.timeout).map_err(|error| fault("provider", error))?;
    let closed = github::call("issue.close", || client.close(&issue.node))
        .map_err(provider)?
        .value;
    let identity = closed.id == issue.node && closed.repository.id == issue.repository;
    let lifecycle = closed.state == "CLOSED" && closed.reason == "COMPLETED";
    if !identity || !lifecycle {
        return Err(fault(
            "reply",
            "close acknowledgement does not bind the expected Issue and completed lifecycle",
        ));
    }
    Ok(())
}

fn fault(kind: &str, message: impl Into<String>) -> Error {
    Error::typed(format!("concord.issue.close.{kind}"), message)
}

fn provider(error: runseal::github::Error) -> Error {
    fault("provider", github::describe(error))
}
