use super::super::{
    fault, inspect,
    target::Target,
    wire::{self, Client},
};
use super::model::{Action, Intent};
use crate::runtime::github;
use concord_core::Result;
use runseal::github::board::{Detail, Section};
use serde_json::{Value, json};

pub(super) async fn state(
    client: &Client,
    target: &str,
    intent: &Intent,
    pages: u16,
) -> Result<Value> {
    let reader = Reader { client, pages };
    for attempt in 0..3 {
        match reader.state(target, intent).await {
            Err(error) if error.code() == "concord.project.changed" && attempt < 2 => {
                tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            }
            result => return result,
        }
    }
    unreachable!("finite projection acquisition")
}

pub(super) async fn inventory(client: &Client, owner: &str, pages: u16) -> Result<Value> {
    Reader { client, pages }.owner(owner).await
}

struct Reader<'a> {
    client: &'a Client,
    pages: u16,
}

impl Reader<'_> {
    async fn state(&self, target: &str, intent: &Intent) -> Result<Value> {
        let target = match intent.action {
            Action::Create { .. } => return self.owner(target).await,
            _ => Target::parse(target)?,
        };
        let project = &self.client.project(&target).await?;
        let mut state = json!({"project":project});
        for (name, section) in [
            ("fields", Section::Fields),
            ("items", Section::Items(detail(intent))),
            ("views", Section::Views),
            ("workflows", Section::Workflows),
            ("repositories", Section::Repositories),
            ("statusUpdates", Section::Updates),
        ] {
            let mut values =
                wire::collect(self.client, project, section, (100, self.pages)).await?;
            if !values.complete {
                return Err(fault(
                    "incomplete",
                    format!("{name} remains incomplete; no write is prepared"),
                ));
            }
            match name {
                "fields" => inspect::fields(&mut values.nodes, project)?,
                "views" | "workflows" => inspect::configuration(&values.nodes, project, name)?,
                "items" => inspect::membership(&values.nodes, project)?,
                _ => {}
            }
            state[name] = json!(values.nodes);
        }
        let owner = project["owner"]["login"].as_str().expect("qualified owner");
        state["columns"] = json!(
            self.client
                .columns(owner, target.number, self.pages)
                .await?
        );
        columns(&state)?;
        state["issues"] = self.issues(intent).await?;
        state["repository"] = self.repository(intent).await?;
        state["origin"] = if matches!(intent.action, Action::Attach { .. }) {
            super::super::field::origin(self.client, project, intent).await?
        } else {
            self.origin(intent).await?
        };
        Ok(state)
    }

    async fn owner(&self, login: &str) -> Result<Value> {
        let _ = Target::parse(&format!("{login}/1"))?;
        let mut values = wire::Collection::new();
        let mut identity = Value::Null;
        for _ in 0..self.pages {
            let owner = self.client.organization(login, values.after()).await?;
            let current = json!({"id":owner["id"],"login":owner["login"],"url":owner["url"]});
            if !current["id"].is_string()
                || !current["login"]
                    .as_str()
                    .is_some_and(|actual| actual.eq_ignore_ascii_case(login))
            {
                return Err(fault(
                    "identity",
                    "organization identity is missing or foreign",
                ));
            }
            let url = format!(
                "https://github.com/{}",
                current["login"].as_str().expect("qualified login")
            );
            if current["url"] != url {
                return Err(fault(
                    "identity",
                    "organization inventory is on a foreign provider host",
                ));
            }
            if !identity.is_null() && current != identity {
                return Err(fault("changed", "organization identity changed"));
            }
            identity = current;
            values.push(&owner["projectsV2"])?;
            for project in &values.nodes {
                let coordinate = format!("{login}/{}", project["number"]);
                Target::parse(&coordinate)?.verify(project)?;
                if project["owner"]["id"] != identity["id"] {
                    return Err(fault(
                        "identity",
                        "Project inventory included a foreign organization owner",
                    ));
                }
            }
            if values.complete {
                return Ok(json!({"owner":identity,"projects":values.nodes}));
            }
        }
        Err(fault(
            "incomplete",
            "organization Project inventory is truncated",
        ))
    }

    async fn issues(&self, intent: &Intent) -> Result<Value> {
        let Action::Candidates { issues, .. } = &intent.action else {
            return Ok(Value::Null);
        };
        if issues.is_empty() || issues.len() > 500 {
            return Err(fault(
                "bounds",
                "candidate change requires 1..500 stable Issue identities",
            ));
        }
        let native = self.client.open()?;
        let mut nodes = Vec::new();
        for ids in issues.chunks(50) {
            let values = github::call("project.candidates", || native.candidates(ids))
                .map_err(wire::provider)?
                .value;
            if values.len() != ids.len() {
                return Err(fault(
                    "identity",
                    "candidate Issue response omitted a requested identity",
                ));
            }
            nodes.extend(values);
        }
        Ok(json!(nodes))
    }

    async fn repository(&self, intent: &Intent) -> Result<Value> {
        let Action::Link { repository, .. } = &intent.action else {
            return Ok(Value::Null);
        };
        let native = self.client.open()?;
        Ok(github::call("project.seat", || native.seat(repository))
            .map_err(wire::provider)?
            .value)
    }

    async fn origin(&self, intent: &Intent) -> Result<Value> {
        let Action::Detach { origin, .. } = &intent.action else {
            return Ok(Value::Null);
        };
        let native = self.client.open()?;
        let node = github::call("project.exists", || native.exists(origin))
            .map_err(wire::provider)?
            .value;
        if node["id"] != *origin {
            return Err(fault(
                "identity",
                "attached organization Issue field origin is unreadable",
            ));
        }
        Ok(node)
    }
}

fn detail(intent: &Intent) -> Detail {
    match intent.action {
        Action::Value { .. } => Detail::Values,
        Action::Planning { .. } => Detail::Planning,
        Action::Remove { .. } => Detail::Removal,
        _ => Detail::Manage,
    }
}

fn columns(state: &Value) -> Result<()> {
    let mut ids = std::collections::BTreeSet::new();
    for field in state["columns"].as_array().expect("REST fields") {
        if !field["node_id"].is_string() || !field["id"].is_u64() {
            return Err(fault(
                "identity",
                "REST field escaped the qualified Project or omitted its identity",
            ));
        }
        if !ids.insert(field["node_id"].clone().to_string()) {
            return Err(fault("changed", "native REST field identity repeated"));
        }
    }
    let mut gaps = Vec::new();
    inspect::gaps(&state["views"], "views", &mut gaps)?;
    if !gaps.is_empty() {
        return Err(fault(
            "incomplete",
            "native view configuration is truncated or unreadable",
        ));
    }
    Ok(())
}
