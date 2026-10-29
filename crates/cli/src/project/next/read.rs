use super::super::{
    fault, inspect, planning, query,
    target::Target,
    wire::{self, Client, Spec},
};
use crate::args::project::Observe;
use concord_core::Result;
use serde_json::{Value, json};

const ISSUE: &str = "__typename id number title url state updatedAt issueType{id name} repository{id nameWithOwner owner{id}} labels(first:100){totalCount pageInfo{hasNextPage endCursor} nodes{id name}} blockedBy(first:100){totalCount pageInfo{hasNextPage endCursor} nodes{id url state}}";

pub(super) async fn state(client: &Client<'_>, args: &Observe) -> Result<Value> {
    let target = Target::parse(&args.project)?;
    let query = format!(
        "query($owner:String!,$number:Int!){{organization(login:$owner){{projectV2(number:$number){{{}}}}}}}",
        query::META
    );
    let data = client
        .read(&query, json!({"owner":target.owner,"number":target.number}))
        .await?;
    let project = &data["organization"]["projectV2"];
    if project.is_null() {
        return Err(fault(
            "unavailable",
            "Project is missing or inaccessible; this proves no absence",
        ));
    }
    target.verify(project)?;
    if project["owner"]["id"].as_str().is_none_or(str::is_empty) {
        return Err(fault(
            "identity",
            "Project organization identity is unreadable",
        ));
    }
    if !project["closed"].is_boolean() {
        return Err(fault("schema", "Project lifecycle is unreadable"));
    }
    let mut state = json!({"project":project});
    for name in ["fields", "items"] {
        let spec = Spec {
            name,
            query: connection(name),
            size: if name == "items" {
                args.size
            } else {
                args.size.min(20)
            },
            pages: args.pages,
        };
        let mut collection = wire::collect(client, project, spec).await?;
        if !collection.complete {
            return Err(fault(
                "incomplete",
                format!("{name} coverage is incomplete; next cannot decide"),
            ));
        }
        match name {
            "fields" => inspect::fields(&mut collection.nodes, project)?,
            "items" => inspect::membership(&collection.nodes, project)?,
            _ => unreachable!("next connections"),
        }
        state[name] = json!(collection.nodes);
    }
    Ok(state)
}

fn connection(name: &str) -> String {
    let selection = match name {
        "fields" => query::FIELD.to_owned(),
        "items" => planning::selection(&format!(
            "id type isArchived updatedAt project{{id}} content{{__typename ... on Issue{{{ISSUE}}}}}"
        )),
        _ => unreachable!("next connection"),
    };
    format!(
        "query($id:ID!,$size:Int!,$after:String){{node(id:$id){{... on ProjectV2{{{meta} {name}(first:$size,after:$after){{{page} nodes{{{selection}}}}}}}}}}}",
        meta = query::META,
        page = query::PAGE
    )
}

pub(super) async fn confirm(client: &Client<'_>, issue: &Value) -> Result<()> {
    let query = format!("query($id:ID!){{node(id:$id){{... on Issue{{{ISSUE}}}}}}}");
    let data = client.read(&query, json!({"id":issue["id"]})).await?;
    if data["node"] != *issue {
        return Err(fault(
            "changed",
            "selected Issue conditions changed or became inaccessible; read again",
        ));
    }
    Ok(())
}

pub(super) async fn finish(client: &Client<'_>, project: &Value) -> Result<()> {
    let query = format!(
        "query($id:ID!){{node(id:$id){{... on ProjectV2{{{}}}}}}}",
        query::META
    );
    let data = client.read(&query, json!({"id":project["id"]})).await?;
    if data["node"] != *project {
        return Err(fault(
            "changed",
            "Project planning changed or became inaccessible before next completed",
        ));
    }
    Ok(())
}
