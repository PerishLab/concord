use concord_core::acceptance::Comment;
use concord_core::{Coordinate, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(super) struct Header {
    pub node: String,
    pub repository: String,
    pub coordinate: Coordinate,
    pub state: String,
    pub kind: String,
    pub body: String,
    pub updated: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(super) struct Entry {
    #[serde(rename = "id")]
    pub node: String,
    pub body: String,
    pub url: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(super) struct Snapshot {
    pub header: Header,
    pub labels: Vec<String>,
    pub entries: Vec<Entry>,
}

impl Snapshot {
    pub fn comments(&self) -> Vec<Comment> {
        self.entries
            .iter()
            .map(|entry| Comment {
                node: entry.node.clone(),
                body: entry.body.clone(),
            })
            .collect()
    }

    pub fn review(&self) -> Result<String> {
        let encoded = serde_json::to_string(&self.header)
            .map_err(|error| super::fault("encode", error.to_string()))?;
        Ok(concord_core::acceptance::digest(&encoded))
    }
}

#[derive(Deserialize)]
pub(super) struct Envelope {
    pub data: Option<Data>,
    pub errors: Option<serde_json::Value>,
}

#[derive(Deserialize)]
pub(super) struct Data {
    pub repository: Option<Repository>,
}

#[derive(Deserialize)]
pub(super) struct Repository {
    pub id: String,
    pub issue: Option<Issue>,
}

#[derive(Deserialize)]
pub(super) struct Issue {
    pub id: String,
    pub number: i64,
    pub url: String,
    pub state: String,
    pub body: String,
    pub updated: String,
    pub kind: Option<Kind>,
    pub labels: Connection<Label>,
    pub comments: Connection<Entry>,
}

#[derive(Deserialize)]
pub(super) struct Kind {
    pub name: String,
}

#[derive(Deserialize)]
pub(super) struct Label {
    pub name: String,
}

#[derive(Deserialize)]
pub(super) struct Connection<T> {
    pub total: usize,
    pub nodes: Vec<T>,
    pub page: Page,
}

#[derive(Deserialize)]
pub(super) struct Page {
    pub next: bool,
    pub cursor: Option<String>,
}
