pub(in crate::project) mod query;

use super::fault;
use crate::runtime::transport::Request;
use concord_core::Result;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::path::Path;

pub(super) struct Client<'a> {
    pub command: &'a Path,
    pub timeout: u64,
}

impl Client<'_> {
    pub async fn read(&self, query: &str, variables: Value) -> Result<Value> {
        let input = serde_json::to_vec(&json!({"query": query, "variables": variables}))
            .expect("fixed GraphQL request");
        let reply = Request::new(self.command, self.timeout, 4 * 1024 * 1024)
            .args(["api", "graphql", "--input", "-"])
            .input(&input)
            .run()
            .await
            .map_err(|error| fault("provider", error.to_string()))?;
        let value: Value = serde_json::from_slice(&reply.stdout)
            .map_err(|error| fault("schema", format!("invalid provider JSON: {error}")))?;
        if let Some(errors) = value.get("errors")
            && errors.as_array().is_none_or(|errors| !errors.is_empty())
        {
            return Err(fault(
                "provider",
                format!("GraphQL errors; partial data is not absence: {errors}"),
            ));
        }
        value
            .get("data")
            .filter(|data| data.is_object())
            .cloned()
            .ok_or_else(|| fault("schema", "provider response has no data object"))
    }
}

pub(super) struct Collection {
    pub nodes: Vec<Value>,
    pub pages: Vec<Value>,
    pub complete: bool,
    cursor: Option<String>,
    total: Option<u64>,
    seen: BTreeSet<String>,
}

impl Collection {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            pages: Vec::new(),
            complete: false,
            cursor: None,
            total: None,
            seen: BTreeSet::new(),
        }
    }

    pub fn after(&self) -> Value {
        json!(self.cursor)
    }

    pub fn push(&mut self, connection: &Value) -> Result<()> {
        let total = connection["totalCount"]
            .as_u64()
            .ok_or_else(|| fault("schema", "connection has no native total"))?;
        let more = connection["pageInfo"]["hasNextPage"]
            .as_bool()
            .ok_or_else(|| fault("schema", "connection has no continuation flag"))?;
        let cursor = connection["pageInfo"]["endCursor"]
            .as_str()
            .map(str::to_owned);
        let nodes = connection["nodes"]
            .as_array()
            .ok_or_else(|| fault("schema", "connection nodes are unreadable"))?;
        if self.total.is_some_and(|previous| previous != total) {
            return Err(fault(
                "changed",
                "connection total changed while paging; observe again",
            ));
        }
        let stuck = nodes.is_empty() || cursor.is_none() || cursor == self.cursor;
        if more && stuck {
            return Err(fault(
                "schema",
                "connection does not advance its continuation cursor",
            ));
        }
        for node in nodes {
            let id = node["id"]
                .as_str()
                .filter(|id| !id.is_empty())
                .ok_or_else(|| fault("schema", "connection member identity is unreadable"))?;
            if !self.seen.insert(id.into()) {
                return Err(fault(
                    "changed",
                    "connection repeated a member identity while paging",
                ));
            }
        }
        self.pages.push(json!({"requested_after": self.cursor, "end_cursor": cursor, "has_next_page": more, "provider_total": total}));
        self.nodes.extend(nodes.iter().cloned());
        self.cursor = cursor;
        self.total = Some(total);
        self.complete = !more;
        if self.nodes.len() as u64 > total || (!more && self.nodes.len() as u64 != total) {
            return Err(fault(
                "changed",
                "connection count disagrees with native total",
            ));
        }
        Ok(())
    }

    pub fn value(&self) -> Value {
        json!({"nodes": self.nodes, "pages": self.pages, "complete": self.complete, "provider_total": self.total})
    }
}

pub(super) fn metadata(value: &Value) -> Value {
    let mut value = value.clone();
    if let Some(object) = value.as_object_mut() {
        for name in [
            "fields",
            "items",
            "views",
            "workflows",
            "repositories",
            "statusUpdates",
        ] {
            object.remove(name);
        }
    }
    value
}

pub(super) struct Spec<'a> {
    pub name: &'a str,
    pub query: String,
    pub size: u16,
    pub pages: u16,
}

pub(super) async fn gather(
    client: &Client<'_>,
    project: &Value,
    name: &str,
    budget: (u16, u16),
) -> Result<Collection> {
    collect(
        client,
        project,
        Spec {
            name,
            query: query::connection(name),
            size: budget.0,
            pages: budget.1,
        },
    )
    .await
}

pub(super) async fn collect(
    client: &Client<'_>,
    project: &Value,
    spec: Spec<'_>,
) -> Result<Collection> {
    let mut collection = Collection::new();
    let query = spec.query;
    for _ in 0..spec.pages {
        let data = client
            .read(
                &query,
                json!({"id": project["id"], "size": spec.size, "after": collection.after()}),
            )
            .await?;
        let node = &data["node"];
        if metadata(node) != *project {
            return Err(fault(
                "changed",
                "Project metadata changed or became inaccessible while observing",
            ));
        }
        collection.push(&node[spec.name])?;
        if collection.complete {
            break;
        }
    }
    Ok(collection)
}
