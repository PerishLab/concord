use concord_core::{Error, Repository, Result};
use serde::Deserialize;
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

const LIMIT: usize = 1024 * 1024;

#[derive(Debug, Eq, PartialEq)]
pub enum Observed {
    Absent,
    Issue { open: bool, kind: Option<String> },
}

#[derive(Deserialize)]
struct Envelope {
    data: Option<Data>,
    #[serde(default)]
    errors: Vec<Failure>,
}

#[derive(Deserialize)]
struct Data {
    repository: Option<Map<String, Value>>,
}

#[derive(Deserialize)]
struct Failure {
    message: String,
    #[serde(default, rename = "type")]
    kind: Option<String>,
    #[serde(default)]
    path: Vec<Value>,
}

#[derive(Deserialize)]
struct Node {
    #[serde(rename = "__typename")]
    class: String,
    #[serde(default)]
    state: Option<String>,
    #[serde(default, rename = "issueType")]
    kind: Option<Named>,
}

#[derive(Deserialize)]
struct Named {
    name: String,
}

pub async fn states(
    repository: &Repository,
    numbers: &BTreeSet<u64>,
    command: &Path,
    timeout: u64,
) -> Result<BTreeMap<u64, Observed>> {
    if numbers.is_empty() {
        return Ok(BTreeMap::new());
    }
    let fields = numbers
        .iter()
        .map(|number| {
            format!(
                "i{number}:issueOrPullRequest(number:{number}){{__typename ... on Issue{{state issueType{{name}}}}}}"
            )
        })
        .collect::<String>();
    let query = format!(
        "query Branches($owner:String!,$name:String!){{repository(owner:$owner,name:$name){{{fields}}}}}"
    );
    let output = super::transport::Request::new(command, timeout, LIMIT)
        .args(["api", "graphql", "-f"])
        .arg(format!("query={query}"))
        .args(["-f", &format!("owner={}", repository.owner)])
        .args(["-f", &format!("name={}", repository.name)])
        .run()
        .await
        .map_err(|failure| provider(failure.to_string()))?;
    decode(&output.stdout, numbers)
}

fn decode(body: &[u8], numbers: &BTreeSet<u64>) -> Result<BTreeMap<u64, Observed>> {
    let envelope = serde_json::from_slice::<Envelope>(body)
        .map_err(|error| provider(format!("Issue branch query did not answer JSON: {error}")))?;
    let mut absent = BTreeSet::new();
    for failure in &envelope.errors {
        match (failure.kind.as_deref(), failure.path.as_slice()) {
            (Some("NOT_FOUND"), [Value::String(scope), Value::String(alias)])
                if scope == "repository" =>
            {
                absent.insert(alias.clone());
            }
            _ => {
                return Err(provider(format!(
                    "Issue branch query was refused: {}",
                    failure.message
                )));
            }
        }
    }
    let repository = envelope
        .data
        .and_then(|data| data.repository)
        .ok_or_else(|| provider("Issue branch query read no repository"))?;
    let mut observed = BTreeMap::new();
    for number in numbers {
        let alias = format!("i{number}");
        let node = match repository.get(&alias) {
            Some(Value::Null) if absent.contains(&alias) => None,
            Some(Value::Null) | None => {
                return Err(provider(format!(
                    "Issue branch query omitted Issue {number}"
                )));
            }
            Some(value) => Some(serde_json::from_value::<Node>(value.clone()).map_err(
                |error| {
                    provider(format!(
                        "Issue branch query answered a malformed node: {error}"
                    ))
                },
            )?),
        };
        observed.insert(*number, shape(node));
    }
    Ok(observed)
}

fn shape(node: Option<Node>) -> Observed {
    match node {
        Some(node) if node.class == "Issue" => Observed::Issue {
            open: node.state.as_deref() == Some("OPEN"),
            kind: node.kind.map(|kind| kind.name),
        },
        _ => Observed::Absent,
    }
}

fn provider(message: impl Into<String>) -> Error {
    Error::typed("concord.delivery.provider", message)
}

#[cfg(test)]
mod tests {
    use super::Observed;
    use std::collections::BTreeSet;

    #[test]
    fn decode() {
        let numbers = BTreeSet::from([1, 2, 3]);
        let body = br#"{"data":{"repository":{"i1":{"__typename":"Issue","state":"OPEN","issueType":{"name":"Task"}},"i2":{"__typename":"PullRequest"},"i3":null}},"errors":[{"type":"NOT_FOUND","path":["repository","i3"],"message":"missing"}]}"#;
        let observed = super::decode(body, &numbers).expect("decoded");
        assert_eq!(
            observed[&1],
            Observed::Issue {
                open: true,
                kind: Some("Task".into())
            }
        );
        assert_eq!(observed[&2], Observed::Absent);
        assert_eq!(observed[&3], Observed::Absent);
        let refused = br#"{"data":null,"errors":[{"type":"RATE_LIMITED","message":"slow down"}]}"#;
        assert_eq!(
            super::decode(refused, &numbers)
                .expect_err("refused")
                .code(),
            "concord.delivery.provider"
        );
        let silent = br#"{"data":{"repository":{"i1":null,"i2":null,"i3":null}}}"#;
        assert_eq!(
            super::decode(silent, &numbers).expect_err("silent").code(),
            "concord.delivery.provider"
        );
    }
}
