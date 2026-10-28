use super::super::{
    fault, inspect, query,
    target::Target,
    wire::{self, Client, Spec},
};
use super::model::{Action, Intent};
use super::rest;
use concord_core::Result;
use serde_json::{Value, json};

pub(super) async fn state(
    client: &Client<'_>,
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
pub(super) async fn inventory(client: &Client<'_>, owner: &str, pages: u16) -> Result<Value> {
    Reader { client, pages }.owner(owner).await
}
struct Reader<'a, 'b> {
    client: &'a Client<'b>,
    pages: u16,
}
impl Reader<'_, '_> {
    async fn state(&self, target: &str, intent: &Intent) -> Result<Value> {
        let target = match intent.action {
            Action::Create { .. } => return self.owner(target).await,
            _ => Target::parse(target)?,
        };
        let query = "query($owner:String!,$number:Int!){organization(login:$owner){projectV2(number:$number){$META}}}".replace("$META", query::META);
        let data = self
            .client
            .read(&query, json!({"owner":target.owner,"number":target.number}))
            .await?;
        let project = &data["organization"]["projectV2"];
        target.verify(project)?;
        let mut state = json!({"project":project});
        for name in [
            "fields",
            "items",
            "views",
            "workflows",
            "repositories",
            "statusUpdates",
        ] {
            let spec = Spec {
                name,
                query: connection(name, intent),
                size: 100,
                pages: self.pages,
            };
            let mut values = wire::collect(self.client, project, spec).await?;
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
        let route = format!(
            "orgs/{}/projectsV2/{}/fields",
            project["owner"]["login"].as_str().expect("qualified owner"),
            target.number
        );
        state["columns"] = json!(rest::fields(self.client, &route, self.pages).await?);
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
            let data = self.client.read("query($owner:String!,$after:String){organization(login:$owner){id login url projectsV2(first:100,after:$after){totalCount pageInfo{hasNextPage endCursor} nodes{__typename id number title url public updatedAt owner{__typename ... on Organization{id login}}}}}}",json!({"owner":login,"after":values.after()})).await?;
            let owner = &data["organization"];
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
        let mut nodes = Vec::new();
        for ids in issues.chunks(50) {
            let data = self.client.read("query($ids:[ID!]!){nodes(ids:$ids){__typename ... on Issue{id number url state updatedAt issueType{id name} repository{id owner{id}}}}}", json!({"ids":ids})).await?;
            let values = data["nodes"]
                .as_array()
                .ok_or_else(|| fault("schema", "candidate Issue facts are unreadable"))?;
            if values.len() != ids.len() {
                return Err(fault(
                    "identity",
                    "candidate Issue response omitted a requested identity",
                ));
            }
            nodes.extend(values.iter().cloned());
        }
        Ok(json!(nodes))
    }

    async fn repository(&self, intent: &Intent) -> Result<Value> {
        let Action::Link { repository, .. } = &intent.action else {
            return Ok(Value::Null);
        };
        let data = self.client.read("query($id:ID!){node(id:$id){__typename ... on Repository{id nameWithOwner owner{id}}}}",json!({"id":repository})).await?;
        Ok(data["node"].clone())
    }

    async fn origin(&self, intent: &Intent) -> Result<Value> {
        let Action::Detach { origin, .. } = &intent.action else {
            return Ok(Value::Null);
        };
        let data = self
            .client
            .read(
                "query($id:ID!){node(id:$id){__typename ... on Node{id}}}",
                json!({"id":origin}),
            )
            .await?;
        if data["node"]["id"] != *origin {
            return Err(fault(
                "identity",
                "attached organization Issue field origin is unreadable",
            ));
        }
        Ok(data["node"].clone())
    }
}

fn connection(name: &str, intent: &Intent) -> String {
    let selection = match name {
        "fields" => query::FIELD,
        "items" => {
            "id type isArchived updatedAt project{id} content{__typename ... on Issue{id number url title state updatedAt issueType{id name} repository{id nameWithOwner owner{id}}}}"
        }
        "views" => {
            "id fullDatabaseId number name layout filter updatedAt project{id} configuration{visibleFields(first:100){pageInfo{hasNextPage endCursor} nodes{... on ProjectV2FieldCommon{id}}}} groupByFields(first:100){totalCount pageInfo{hasNextPage endCursor} nodes{... on ProjectV2FieldCommon{id}}} verticalGroupByFields(first:100){totalCount pageInfo{hasNextPage endCursor} nodes{... on ProjectV2FieldCommon{id}}} sortByFields(first:100){totalCount pageInfo{hasNextPage endCursor} nodes{direction field{... on ProjectV2FieldCommon{id}}}}"
        }
        "workflows" => "id name enabled updatedAt project{id}",
        "repositories" => "id nameWithOwner url owner{id}",
        "statusUpdates" => "id body createdAt updatedAt",
        _ => unreachable!("fixed native planning surface"),
    };
    let selection = if name == "items" && matches!(intent.action, Action::Planning { .. }) {
        super::super::planning::selection(selection)
    } else if name == "items" && matches!(intent.action, Action::Remove { .. }) {
        values(selection)
    } else {
        selection.to_string()
    };
    format!(
        "query($id:ID!,$size:Int!,$after:String){{node(id:$id){{... on ProjectV2{{{meta} {name}(first:$size,after:$after){{{page} nodes{{{selection}}}}}}}}}}}",
        meta = query::META,
        page = query::PAGE
    )
}

fn columns(state: &Value) -> Result<()> {
    let url = format!(
        "https://api.github.com/orgs/{}/projectsV2/{}",
        state["project"]["owner"]["login"]
            .as_str()
            .expect("qualified owner"),
        state["project"]["number"]
    );
    let mut ids = std::collections::BTreeSet::new();
    for field in state["columns"].as_array().expect("REST fields") {
        if field["project_url"] != url || !field["node_id"].is_string() || !field["id"].is_u64() {
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

fn values(item: &str) -> String {
    let kinds = [
        "Date",
        "Iteration",
        "Label",
        "Milestone",
        "MultiSelect",
        "Number",
        "PullRequest",
        "Repository",
        "Reviewer",
        "SingleSelect",
        "Text",
        "User",
    ];
    let fragments = kinds
        .iter()
        .map(|kind| {
            format!(
                "... on ProjectV2ItemField{kind}Value{{field{{... on ProjectV2FieldCommon{{id}}}}}}"
            )
        })
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "{item} fieldValues(first:100){{totalCount pageInfo{{hasNextPage endCursor}} nodes{{__typename {fragments} ... on ProjectV2ItemIssueFieldValue{{field{{... on ProjectV2FieldCommon{{id}}}}}}}}}}"
    )
}
