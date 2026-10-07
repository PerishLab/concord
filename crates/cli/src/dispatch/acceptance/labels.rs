use concord_core::acceptance::Target;
use concord_core::{Coordinate, Result};
use serde_json::{Value, json};

use super::super::forge::Request;
use crate::args::acceptance::Observe;

const LOOKUP: &str = "query($owner:String!,$name:String!,$label:String!){repository(owner:$owner,name:$name){id label(name:$label){id name}}}";
const CREATE: &str = "mutation($repository:ID!,$name:String!){created:createLabel(input:{repositoryId:$repository,name:$name,color:\"5319e7\"}){label{id name}}}";
const ADD: &str = "mutation($issue:ID!,$labels:[ID!]!){alter:addLabelsToLabelable(input:{labelableId:$issue,labelIds:$labels}){labelable{id}}}";
const REMOVE: &str = "mutation($issue:ID!,$labels:[ID!]!){alter:removeLabelsFromLabelable(input:{labelableId:$issue,labelIds:$labels}){labelable{id}}}";

pub(super) async fn resolve(
    args: &Observe,
    repository: &str,
    target: Target,
    create: bool,
) -> Result<String> {
    let coordinate = Coordinate::parse(&args.issue)?;
    let variables =
        json!({"owner": coordinate.owner, "name": coordinate.repository, "label": target.label()});
    let value = request(args, LOOKUP, variables).await?;
    let observed = value
        .pointer("/data/repository")
        .ok_or_else(|| super::fault("reply", "label repository is unreadable"))?;
    if observed["id"] != repository {
        return Err(super::fault("reply", "label repository identity differs"));
    }
    let label = observed
        .get("label")
        .ok_or_else(|| super::fault("reply", "label lookup omitted its result"))?;
    if !label.is_null() {
        return identity(label, target);
    }
    if !create {
        return Err(super::fault(
            "missing",
            "managed predecessor label is unreadable",
        ));
    }
    let variables = json!({"repository": repository, "name": target.label()});
    let value = request(args, CREATE, variables).await?;
    let label = value
        .pointer("/data/created/label")
        .ok_or_else(|| super::fault("reply", "label creation named no label"))?;
    identity(label, target)
}

pub(super) async fn alter(args: &Observe, issue: &str, label: &str, remove: bool) -> Result<()> {
    let query = if remove { REMOVE } else { ADD };
    let value = request(args, query, json!({"issue": issue, "labels": [label]})).await?;
    if value.pointer("/data/alter/labelable/id") != Some(&json!(issue)) {
        return Err(super::fault(
            "reply",
            "label mutation named another or no Issue",
        ));
    }
    Ok(())
}

fn identity(value: &Value, target: Target) -> Result<String> {
    if value["name"] != target.label() {
        return Err(super::fault(
            "reply",
            "provider named another managed label",
        ));
    }
    value["id"]
        .as_str()
        .filter(|id| !id.trim().is_empty())
        .map(str::to_string)
        .ok_or_else(|| super::fault("reply", "managed label has no identity"))
}

async fn request(args: &Observe, query: &str, variables: Value) -> Result<Value> {
    let input = serde_json::to_vec(&json!({"query": query, "variables": variables}))
        .map_err(|error| super::fault("encode", error.to_string()))?;
    let reply = Request::new(&args.command, args.timeout, 16 * 1024)
        .args(["api", "graphql", "--input", "-"])
        .input(&input)
        .run()
        .await
        .map_err(|error| super::fault("provider", error.to_string()))?;
    let value: Value = serde_json::from_slice(&reply.stdout)
        .map_err(|error| super::fault("reply", error.to_string()))?;
    if value.get("errors").is_some() || !value["data"].is_object() {
        return Err(super::fault(
            "provider",
            "label reply contains errors or no data",
        ));
    }
    Ok(value)
}
