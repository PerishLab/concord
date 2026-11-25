use super::fault;
use super::wire::Client;
use crate::runtime::github;
use concord_core::{Coordinate, Result};
use runseal::github::{Cause, Number, Target};
use serde_json::{Value, json};
use std::collections::BTreeSet;

#[cfg(all(test, unix))]
mod fixture;
#[cfg(all(test, unix))]
mod tests;

pub(super) async fn observe(client: &Client, collections: &mut Value, pages: u16) -> Result<()> {
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
        client: *client,
        origins,
        pages,
    }
    .observe(collections)
    .await
}

struct Reader {
    client: Client,
    origins: Vec<Value>,
    pages: u16,
}

impl Reader {
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
        let repository = issue["repository"]["nameWithOwner"]
            .as_str()
            .expect("validated Issue repository");
        let number = issue["number"].as_u64().expect("validated Issue number");
        let coordinate = Coordinate::parse(&format!("{repository}#{number}"))?;
        let target = Target::new(
            &coordinate.owner,
            &coordinate.repository,
            u32::try_from(coordinate.number)
                .map_err(|_| fault("schema", "invalid Issue number"))?,
        )
        .map_err(translate)?;
        let id = issue["id"]
            .as_str()
            .ok_or_else(|| fault("schema", "Issue identity is unreadable"))?
            .to_owned();
        let updated = issue["updatedAt"].clone();
        let seat = issue["repository"]["id"].clone();
        let client = self.client;
        let pages = self.pages;
        tokio::task::spawn_blocking(move || read(client, pages, &target, (&id, &updated, &seat)))
            .await
            .map_err(|error| fault("provider", error.to_string()))?
    }
}

fn read(
    client: Client,
    pages: u16,
    target: &Target,
    expected: (&str, &Value, &Value),
) -> Result<Value> {
    let native = client.open()?;
    let mut nodes = Vec::new();
    let mut seen = BTreeSet::new();
    let mut complete = false;
    let mut history = Vec::new();
    for page in 1..=pages {
        let window = Number::new(u32::from(page), 100).map_err(translate)?;
        let listing = github::call("issue.attached", || native.attached(target, window))
            .map_err(translate)?
            .value;
        let count = listing.rows.len();
        for entry in listing.rows {
            if !seen.insert(entry.field) {
                return Err(fault(
                    "changed",
                    "Issue field identity repeated while paging",
                ));
            }
            let value =
                serde_json::to_value(entry).map_err(|error| fault("schema", error.to_string()))?;
            nodes.push(value);
        }
        complete = !listing.more;
        history.push(json!({"requested_page": page, "per_page": 100, "returned": count, "may_have_more": !complete}));
        if complete {
            break;
        }
    }
    let subject = github::call("project.subject", || native.subject(expected.0))
        .map_err(|error| {
            let code = if error.cause == Cause::Shape {
                "changed"
            } else {
                "provider"
            };
            fault(code, github::describe(error))
        })?
        .value;
    if subject.id != expected.0
        || *expected.1 != subject.updated
        || *expected.2 != subject.repository.id
    {
        return Err(fault(
            "changed",
            "Issue changed or became inaccessible while reading attached field values",
        ));
    }
    Ok(json!({"authority": "issue", "complete": complete, "pages": history, "nodes": nodes}))
}

fn translate(error: runseal::github::Error) -> concord_core::Error {
    let code = if error.cause == Cause::Shape {
        "schema"
    } else {
        "provider"
    };
    fault(code, github::describe(error))
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
