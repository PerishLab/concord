use crate::args::close::Observe;
use crate::dispatch::forge::close::model::{Issue, Reason, State};
use concord_core::{Coordinate, Error, Result};
use serde::Deserialize;
use serde_json::{Value, json};

const QUERY: &str = "query ConcordIssueClose($owner:String!,$name:String!,$number:Int!){repository(owner:$owner,name:$name){id nameWithOwner issue(number:$number){id number url body state reason:stateReason updated:updatedAt writable:viewerCanUpdate kind:issueType{name}}}}";
const MUTATION: &str = "mutation ConcordIssueClose($id:ID!){closeIssue(input:{issueId:$id,stateReason:COMPLETED}){issue{id state reason:stateReason repository{id}}}}";

#[derive(Deserialize)]
struct Envelope<T> {
    data: Option<T>,
    errors: Option<Value>,
}
#[derive(Deserialize)]
struct Data {
    repository: Repository,
}
#[derive(Deserialize)]
struct Repository {
    id: String,
    #[serde(rename = "nameWithOwner")]
    name: String,
    issue: Wire,
}
#[derive(Deserialize)]
struct Wire {
    id: String,
    number: i64,
    url: String,
    body: String,
    state: String,
    reason: Value,
    updated: String,
    writable: bool,
    kind: Option<Named>,
}
#[derive(Deserialize)]
struct Named {
    name: String,
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
    let data: Data = exchange(
        args,
        QUERY,
        json!({"owner":coordinate.owner,"name":coordinate.repository,"number":coordinate.number}),
    )
    .await?;
    let repository = data.repository;
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
            kind: raw.kind.map(|kind| kind.name).unwrap_or_default(),
            state,
            reason,
            review: concord_core::acceptance::digest(&raw.body),
            updated: raw.updated,
            writable: raw.writable,
        },
        body: raw.body,
    })
}

pub(in crate::dispatch::forge) async fn close(args: &Observe, issue: &Issue) -> Result<()> {
    let data: Value = exchange(args, MUTATION, json!({"id":issue.node})).await?;
    let closed = &data["closeIssue"]["issue"];
    let identity = closed["id"] == issue.node && closed["repository"]["id"] == issue.repository;
    let lifecycle = closed["state"] == "CLOSED" && closed["reason"] == "COMPLETED";
    if !identity || !lifecycle {
        return Err(fault(
            "reply",
            "close acknowledgement does not bind the expected Issue and completed lifecycle",
        ));
    }
    Ok(())
}

async fn exchange<T: serde::de::DeserializeOwned>(
    args: &Observe,
    query: &str,
    variables: Value,
) -> Result<T> {
    let input = serde_json::to_vec(&json!({"query":query,"variables":variables}))
        .expect("fixed closure request");
    let reply = super::transport::Request::new(&args.command, args.timeout, 1024 * 1024)
        .args(["api", "graphql", "--input", "-"])
        .input(&input)
        .run()
        .await
        .map_err(|_| {
            fault(
                "provider",
                "GitHub exchange did not yield a complete closure response",
            )
        })?;
    let envelope: Envelope<T> = serde_json::from_slice(&reply.stdout)
        .map_err(|error| fault("schema", format!("invalid closure response: {error}")))?;
    if envelope.errors.is_some() {
        return Err(fault(
            "provider",
            "GitHub returned errors; partial data is unknown",
        ));
    }
    envelope
        .data
        .ok_or_else(|| fault("provider", "GitHub closure response omitted current facts"))
}

fn fault(kind: &str, message: impl Into<String>) -> Error {
    Error::typed(format!("concord.issue.close.{kind}"), message)
}
