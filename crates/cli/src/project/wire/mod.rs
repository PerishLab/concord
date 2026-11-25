use super::{fault, target::Target};
use crate::runtime::github;
use concord_core::Result;
use runseal::github::board::Section;
use runseal::github::{Organization, Overview, Window, columns::Locator};
use serde_json::{Value, json};
use std::collections::BTreeSet;

#[derive(Clone, Copy)]
pub(super) struct Client {
    pub timeout: u64,
    #[cfg(test)]
    pub loopback: Option<std::net::SocketAddr>,
}

impl Client {
    pub fn new(timeout: u64) -> Self {
        Self {
            timeout,
            #[cfg(test)]
            loopback: None,
        }
    }

    pub fn open(&self) -> Result<runseal::github::Client> {
        #[cfg(test)]
        if let Some(address) = self.loopback {
            return github::loopback(address, self.timeout)
                .map_err(|error| fault("provider", error));
        }
        github::client(self.timeout).map_err(|error| fault("provider", error))
    }

    pub async fn project(&self, target: &Target) -> Result<Value> {
        let native = self.open()?;
        let held = github::call("project.overview", || {
            native.overview(&target.owner, target.number)
        })
        .map_err(provider)?
        .value;
        let project = overview(held);
        target.verify(&project)?;
        Ok(project)
    }

    pub async fn metadata(&self, project: &str) -> Result<Value> {
        let native = self.open()?;
        let held = github::call("project.metadata", || native.metadata(project))
            .map_err(provider)?
            .value;
        Ok(overview(held))
    }

    pub async fn organization(&self, login: &str, after: Value) -> Result<Value> {
        let native = self.open()?;
        let window = Window::new(100, after.as_str()).map_err(provider)?;
        let held = github::call("project.projects", || native.projects(login, &window))
            .map_err(provider)?
            .value;
        if held.projects.target != held.owner.id {
            return Err(fault(
                "identity",
                "organization inventory page belongs to a different owner",
            ));
        }
        let entries = held
            .projects
            .rows
            .into_iter()
            .map(|project| {
                json!({
                    "__typename":project.kind, "id":project.id, "number":project.number,
                    "title":project.title, "url":project.url, "public":project.public,
                    "updatedAt":project.updated, "owner":identity(project.owner)
                })
            })
            .collect::<Vec<_>>();
        Ok(json!({
            "id":held.owner.id, "login":held.owner.login, "url":held.url,
            "projectsV2":{
                "totalCount":held.projects.total, "nodes":entries,
                "pageInfo":{"hasNextPage":held.projects.next.is_some(),"endCursor":held.projects.next}
            }
        }))
    }

    pub async fn locator(&self, owner: &str, number: u32) -> Result<Locator> {
        let native = self.open()?;
        let account = github::call("project.account", || native.account(owner))
            .map_err(provider)?
            .value;
        Locator::new(&account.login, number, account.database).map_err(provider)
    }

    pub async fn columns(&self, owner: &str, number: u32, pages: u16) -> Result<Vec<Value>> {
        let native = self.open()?;
        let location = self.locator(owner, number).await?;
        let mut cursor: Option<String> = None;
        let mut seen = BTreeSet::new();
        let mut nodes = Vec::new();
        for _ in 0..pages {
            let window = Window::new(100, cursor.as_deref()).map_err(provider)?;
            let slice = github::call("project.columns", || native.columns(&location, &window))
                .map_err(provider)?
                .value;
            for column in slice.rows {
                nodes.push(json!({"id":column.id,"node_id":column.node,"name":column.name}));
            }
            let Some(next) = slice.next else {
                return Ok(nodes);
            };
            if !seen.insert(next.clone()) {
                return Err(fault(
                    "changed",
                    "REST field pagination repeated its cursor",
                ));
            }
            cursor = Some(next);
        }
        Err(fault(
            "incomplete",
            "REST field pagination budget exhausted; no write is prepared",
        ))
    }
}

pub(super) fn overview(held: Overview) -> Value {
    json!({
        "__typename":"ProjectV2", "id":held.id, "number":held.number, "title":held.title,
        "url":held.url, "updatedAt":held.updated, "closed":held.closed, "public":held.public,
        "readme":held.readme, "shortDescription":held.description, "viewerCanUpdate":held.writable,
        "owner":identity(held.owner)
    })
}

fn identity(organization: Organization) -> Value {
    json!({"__typename":organization.kind,"id":organization.id,"login":organization.login})
}

pub(super) fn provider(error: runseal::github::Error) -> concord_core::Error {
    fault("provider", github::describe(error))
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

pub(super) async fn collect(
    client: &Client,
    project: &Value,
    section: Section,
    budget: (u16, u16),
) -> Result<Collection> {
    let native = client.open()?;
    let id = project["id"]
        .as_str()
        .ok_or_else(|| fault("identity", "Project identity is unreadable"))?;
    let size = u8::try_from(budget.0).map_err(|_| fault("bounds", "invalid Project page size"))?;
    let mut collection = Collection::new();
    for _ in 0..budget.1 {
        let after = collection.after();
        let window = Window::new(size, after.as_str()).map_err(provider)?;
        let slice = github::call("project.board", || native.board(id, section, &window))
            .map_err(provider)?
            .value;
        if slice.project != *project {
            return Err(fault(
                "changed",
                "Project metadata changed or became inaccessible while observing",
            ));
        }
        collection.push(&slice.connection)?;
        if collection.complete {
            break;
        }
    }
    Ok(collection)
}
