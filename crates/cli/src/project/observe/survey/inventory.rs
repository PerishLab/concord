use super::super::super::{
    fault,
    wire::{self, Client, Collection},
};
use crate::args::project::Survey;
use crate::runtime::github;
use concord_core::Result;
use runseal::github::Window;
use serde_json::{Value, json};
use std::collections::BTreeSet;

pub(super) struct Inventory {
    pub owner: Value,
    pub repositories: Value,
    pub issues: Vec<Value>,
    pub complete: bool,
}

pub(super) async fn read(client: &Client, args: &Survey) -> Result<Inventory> {
    let (owner, repositories) = repositories(client, args).await?;
    let mut complete = repositories.complete;
    let mut rows = Vec::new();
    let mut observed = Vec::new();
    let mut ids = BTreeSet::new();
    for repo in &repositories.nodes {
        let collection = issues(client, repo, args.remote.pages).await?;
        if observed.len() + collection.nodes.len() > args.issues as usize {
            return Err(fault(
                "bounds",
                "Issue count exceeds --max-issues; no full coverage is claimed",
            ));
        }
        complete &= collection.complete;
        for issue in &collection.nodes {
            validate(issue, repo)?;
            if !ids.insert(issue["id"].clone().to_string()) {
                return Err(fault(
                    "changed",
                    "Issue identity repeated across repositories",
                ));
            }
            observed.push(issue.clone());
        }
        rows.push(json!({"repository":repo,"issues":collection.value()}));
    }
    Ok(Inventory {
        owner,
        repositories: json!({"inventory":repositories.value(),"observations":rows}),
        issues: observed,
        complete,
    })
}

async fn repositories(client: &Client, args: &Survey) -> Result<(Value, Collection)> {
    let native = client.open()?;
    let mut collection = Collection::new();
    let mut identity = Value::Null;
    for _ in 0..args.remote.pages {
        let after = collection.after();
        let window = Window::new(100, after.as_str()).map_err(wire::provider)?;
        let catalog = github::call("project.catalog", || native.catalog(&args.owner, &window))
            .map_err(wire::provider)?
            .value;
        let owner = &catalog.owner;
        let current = json!({"id":owner["id"],"login":owner["login"],"url":owner["url"]});
        if current["id"].as_str().is_none_or(str::is_empty)
            || !current["login"]
                .as_str()
                .is_some_and(|login| login.eq_ignore_ascii_case(&args.owner))
            || current["url"]
                != format!(
                    "https://github.com/{}",
                    current["login"].as_str().unwrap_or("")
                )
        {
            return Err(fault(
                "identity",
                "organization is missing, inaccessible or foreign",
            ));
        }
        if !identity.is_null() && current != identity {
            return Err(fault(
                "changed",
                "organization identity changed while paging",
            ));
        }
        identity = current;
        collection.push(&catalog.repositories)?;
        if collection.nodes.len() > usize::from(args.repositories) {
            return Err(fault(
                "bounds",
                "repository count exceeds --max-repositories; no full coverage is claimed",
            ));
        }
        for repo in &collection.nodes {
            repository(repo, &identity)?;
        }
        if collection.complete {
            break;
        }
    }
    Ok((identity, collection))
}

async fn issues(client: &Client, repo: &Value, pages: u16) -> Result<Collection> {
    let native = client.open()?;
    let id = repo["id"]
        .as_str()
        .ok_or_else(|| fault("identity", "repository identity is unreadable"))?;
    let mut collection = Collection::new();
    for _ in 0..pages {
        let after = collection.after();
        let window = Window::new(100, after.as_str()).map_err(wire::provider)?;
        let open = github::call("project.backlog", || native.backlog(id, &window))
            .map_err(wire::provider)?
            .value;
        let current = &open.repository;
        if current["id"] != repo["id"]
            || current["nameWithOwner"] != repo["nameWithOwner"]
            || current["owner"] != repo["owner"]
        {
            return Err(fault(
                "identity",
                "repository changed or became inaccessible while paging Issues",
            ));
        }
        if current["url"] != repo["url"] {
            return Err(fault("identity", "repository URL disagrees while paging"));
        }
        collection.push(&open.issues)?;
        if collection.complete {
            break;
        }
    }
    Ok(collection)
}

fn repository(repo: &Value, owner: &Value) -> Result<()> {
    let name = repo["nameWithOwner"].as_str().unwrap_or("");
    let Some((login, label)) = name.split_once('/') else {
        return Err(fault("identity", "repository coordinate is unreadable"));
    };
    if login != owner["login"].as_str().unwrap_or("") || label.is_empty() || label.contains('/') {
        return Err(fault(
            "identity",
            "repository coordinate escaped the organization",
        ));
    }
    if repo["owner"]["id"] != owner["id"] || repo["url"] != format!("https://github.com/{name}") {
        return Err(fault(
            "identity",
            "repository escaped the selected organization",
        ));
    }
    if !repo["isArchived"].is_boolean() || !repo["hasIssuesEnabled"].is_boolean() {
        return Err(fault(
            "schema",
            "repository visibility of Issue inventory is unknown",
        ));
    }
    Ok(())
}

pub(super) fn validate(issue: &Value, repo: &Value) -> Result<()> {
    let number = issue["number"].as_u64().filter(|number| *number > 0);
    let url = format!(
        "https://github.com/{}/issues/{}",
        repo["nameWithOwner"].as_str().unwrap_or(""),
        number.unwrap_or(0)
    );
    if issue["repository"]["id"] != repo["id"]
        || issue["repository"]["nameWithOwner"] != repo["nameWithOwner"]
        || issue["repository"]["owner"]["id"] != repo["owner"]["id"]
    {
        return Err(fault(
            "identity",
            "Issue coordinate or repository identity disagrees",
        ));
    }
    if number.is_none() || issue["url"] != url {
        return Err(fault(
            "identity",
            "Issue coordinate disagrees with its repository",
        ));
    }
    if issue["state"] != "OPEN"
        || !issue["title"].is_string()
        || issue["updatedAt"].as_str().is_none_or(str::is_empty)
    {
        return Err(fault("schema", "open Issue observation is unreadable"));
    }
    Ok(())
}
