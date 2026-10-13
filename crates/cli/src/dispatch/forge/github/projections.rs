use concord_core::{Error, Repository, Result};
use serde::Deserialize;
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

const LIMIT: usize = 1024 * 1024;

#[derive(Deserialize)]
struct Envelope {
    data: Option<Data>,
    #[serde(default)]
    errors: Vec<Value>,
}

#[derive(Deserialize)]
struct Data {
    repository: Option<Map<String, Value>>,
}

#[derive(Deserialize)]
struct Connection {
    nodes: Vec<Pull>,
    #[serde(rename = "pageInfo")]
    page: Page,
}

#[derive(Deserialize)]
struct Page {
    #[serde(rename = "hasNextPage")]
    more: bool,
}

#[derive(Deserialize)]
pub struct Pull {
    state: String,
    #[serde(rename = "headRefName")]
    branch: String,
    #[serde(rename = "headRefOid")]
    head: String,
    #[serde(rename = "headRepository")]
    origin: Option<Named>,
    #[serde(rename = "baseRepository")]
    target: Named,
    body: String,
}

#[derive(Deserialize)]
struct Named {
    #[serde(rename = "nameWithOwner")]
    name: String,
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
            origin: self.origin.as_ref().map(|origin| origin.name.as_str()),
            target: &self.target.name,
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
    command: &Path,
    timeout: u64,
) -> Result<BTreeMap<String, Vec<Pull>>> {
    if branches.is_empty() {
        return Ok(BTreeMap::new());
    }
    let fields = branches.iter().enumerate().map(|(index, branch)| {
        let head = serde_json::to_string(branch).expect("encode branch string");
        format!("p{index}:pullRequests(first:2,headRefName:{head},states:OPEN){{nodes{{state headRefName headRefOid headRepository{{nameWithOwner}} baseRepository{{nameWithOwner}} body}} pageInfo{{hasNextPage}}}}")
    }).collect::<String>();
    let query = format!(
        "query Projections($owner:String!,$name:String!){{repository(owner:$owner,name:$name){{{fields}}}}}"
    );
    let output = super::transport::Request::new(command, timeout, LIMIT)
        .args(["api", "graphql", "-f"])
        .arg(format!("query={query}"))
        .args(["-f", &format!("owner={}", repository.owner)])
        .args(["-f", &format!("name={}", repository.name)])
        .run()
        .await
        .map_err(|error| provider(error.to_string()))?;
    decode(&output.stdout, branches)
}

fn decode(body: &[u8], branches: &BTreeSet<String>) -> Result<BTreeMap<String, Vec<Pull>>> {
    let envelope: Envelope = serde_json::from_slice(body)
        .map_err(|error| provider(format!("projection query did not answer JSON: {error}")))?;
    if !envelope.errors.is_empty() {
        return Err(provider("projection query was refused"));
    }
    let repository = envelope
        .data
        .and_then(|data| data.repository)
        .ok_or_else(|| provider("projection query read no repository"))?;
    let mut observed = BTreeMap::new();
    for (index, branch) in branches.iter().enumerate() {
        let value = repository
            .get(&format!("p{index}"))
            .ok_or_else(|| provider(format!("projection query omitted {branch}")))?;
        let connection: Connection = serde_json::from_value(value.clone()).map_err(|error| {
            provider(format!(
                "projection query returned malformed {branch}: {error}"
            ))
        })?;
        if connection.page.more || connection.nodes.len() > 2 {
            return Err(provider(format!(
                "projection query for {branch} exceeds its bound"
            )));
        }
        observed.insert(branch.clone(), connection.nodes);
    }
    Ok(observed)
}

fn provider(message: impl Into<String>) -> Error {
    Error::typed("concord.delivery.provider", message)
}
