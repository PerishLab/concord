use super::fault;
use super::wire::Client;
use crate::runtime::transport::Request;
use concord_core::{Coordinate, Result};
use serde_json::{Value, json};
use std::collections::BTreeSet;

#[cfg(all(test, unix))]
mod fixture;
#[cfg(all(test, unix))]
mod tests;

pub(super) async fn observe(
    client: &Client<'_>,
    collections: &mut Value,
    pages: u16,
) -> Result<()> {
    let origins = collections["fields"]["nodes"]
        .as_array()
        .expect("collected fields")
        .iter()
        .filter(|field| field["authority"] == "issue")
        .cloned()
        .collect::<Vec<_>>();
    if origins.is_empty() {
        return Ok(());
    }
    Reader {
        client,
        origins,
        pages,
    }
    .observe(collections)
    .await
}

struct Reader<'a, 'b> {
    client: &'a Client<'b>,
    origins: Vec<Value>,
    pages: u16,
}

impl Reader<'_, '_> {
    async fn observe(&self, collections: &mut Value) -> Result<()> {
        for group in collections["items"]["nodes"]
            .as_array_mut()
            .expect("collected items")
            .chunks_mut(4)
        {
            let (left, right) = group.split_at_mut(group.len().min(2));
            let (a, b) = tokio::join!(self.pair(left), self.pair(right));
            a?;
            b?;
        }
        Ok(())
    }

    async fn pair(&self, items: &mut [Value]) -> Result<()> {
        let mut items = items.iter_mut();
        let (a, b) = tokio::join!(self.item(items.next()), self.item(items.next()));
        a?;
        b
    }

    async fn item(&self, item: Option<&mut Value>) -> Result<()> {
        let Some(item) = item else { return Ok(()) };
        if item["content"]["__typename"] == "Issue" {
            let mut values = self.read(&item["content"]).await?;
            associate(&mut values, &self.origins)?;
            item["content"]["issueFields"] = values;
        }
        Ok(())
    }

    async fn read(&self, issue: &Value) -> Result<Value> {
        let client = self.client;
        let pages = self.pages;
        let repository = issue["repository"]["nameWithOwner"]
            .as_str()
            .expect("validated Issue repository");
        let number = issue["number"].as_u64().expect("validated Issue number");
        Coordinate::parse(&format!("{repository}#{number}"))?;
        let mut nodes = Vec::new();
        let mut seen = BTreeSet::new();
        let mut complete = false;
        let mut history = Vec::new();
        for page in 1..=pages {
            let endpoint = format!(
                "repos/{repository}/issues/{number}/issue-field-values?per_page=100&page={page}"
            );
            let reply = Request::new(client.command, client.timeout, 4 * 1024 * 1024)
                .args(["api", "--method", "GET", &endpoint])
                .run()
                .await
                .map_err(|error| fault("provider", error.to_string()))?;
            let value: Value = serde_json::from_slice(&reply.stdout).map_err(|error| {
                fault(
                    "schema",
                    format!("invalid native Issue field response: {error}"),
                )
            })?;
            let values = value
                .as_array()
                .ok_or_else(|| fault("schema", "Issue field values are unreadable"))?;
            for value in values {
                let id = value["issue_field_id"].as_u64().ok_or_else(|| {
                    fault("schema", "Issue field value has no native field identity")
                })?;
                if !seen.insert(id) {
                    return Err(fault(
                        "changed",
                        "Issue field identity repeated while paging",
                    ));
                }
            }
            complete = values.len() < 100;
            history.push(json!({"requested_page": page, "per_page": 100, "returned": values.len(), "may_have_more": !complete}));
            nodes.extend(values.iter().cloned());
            if complete {
                break;
            }
        }
        let data = client
            .read(
                "query($id:ID!){node(id:$id){... on Issue{id updatedAt repository{id}}}}",
                json!({"id": issue["id"]}),
            )
            .await?;
        if data["node"]["id"] != issue["id"]
            || data["node"]["updatedAt"] != issue["updatedAt"]
            || data["node"]["repository"]["id"] != issue["repository"]["id"]
        {
            return Err(fault(
                "changed",
                "Issue changed or became inaccessible while reading attached field values",
            ));
        }
        Ok(json!({"authority": "issue", "complete": complete, "pages": history, "nodes": nodes}))
    }
}

fn associate(values: &mut Value, origins: &[Value]) -> Result<()> {
    for value in values["nodes"].as_array_mut().expect("native Issue values") {
        let id = value["issue_field_id"]
            .as_u64()
            .expect("validated field identity");
        let fields = origins
            .iter()
            .filter(|field| database(&field["issueField"]["fullDatabaseId"]) == Some(id))
            .collect::<Vec<_>>();
        for field in &fields {
            if value["data_type"].as_str().map(|kind| kind.to_uppercase())
                != field["dataType"].as_str().map(str::to_owned)
            {
                return Err(fault(
                    "schema",
                    "Issue field value type disagrees with attached Project field origin",
                ));
            }
        }
        value["project_fields"] =
            json!(fields.iter().map(|field| &field["id"]).collect::<Vec<_>>());
    }
    Ok(())
}

fn database(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_str().and_then(|value| value.parse().ok()))
}
