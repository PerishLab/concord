mod inspect;
mod issue;
mod query;
mod target;
mod wire;

use crate::args::project::Command;
use concord_core::{Error, Result};
use serde_json::{Value, json};
use std::time::{SystemTime, UNIX_EPOCH};
use target::Target;
use wire::Client;

pub(super) async fn run(command: Command, output: bool) -> Result<()> {
    let Command::Show {
        project,
        command,
        size,
        pages,
        timeout,
    } = command;
    let target = Target::parse(&project)?;
    let client = Client {
        command: &command,
        timeout,
    };
    let started = now();
    let query = "query($owner:String!,$number:Int!){organization(login:$owner){projectV2(number:$number){$META}}}".replace("$META", query::META);
    let data = client
        .read(
            &query,
            json!({"owner": target.owner, "number": target.number}),
        )
        .await?;
    let project = &data["organization"]["projectV2"];
    if project.is_null() {
        return Err(fault(
            "unavailable",
            "Project is missing or inaccessible; this proves no absence",
        ));
    }
    target.verify(project)?;
    let mut collections = serde_json::Map::new();
    let mut diagnostics = Vec::new();
    for name in [
        "fields",
        "items",
        "views",
        "workflows",
        "repositories",
        "statusUpdates",
    ] {
        let bound = if name == "items" { size.min(20) } else { size };
        let mut collection = wire::gather(&client, project, name, (bound, pages)).await?;
        match name {
            "fields" => inspect::fields(&mut collection.nodes, project)?,
            "items" => inspect::items(&mut collection.nodes, project)?,
            "views" | "workflows" => inspect::configuration(&collection.nodes, project, name)?,
            _ => {}
        }
        if !collection.complete {
            diagnostics.push(json!({"code": "page-budget", "connection": name}));
        }
        collections.insert(name.into(), collection.value());
    }
    let mut collections = Value::Object(collections);
    issue::observe(&client, &mut collections, pages).await?;
    let finality = client
        .read(
            &format!(
                "query($id:ID!){{node(id:$id){{... on ProjectV2{{{}}}}}}}",
                query::META
            ),
            json!({"id": project["id"]}),
        )
        .await?;
    if finality["node"] != *project {
        return Err(fault(
            "changed",
            "Project metadata changed or became inaccessible before observation completed",
        ));
    }
    inspect::gaps(&collections, "collections", &mut diagnostics)?;
    inspect::unknown(
        collections["items"]["nodes"]
            .as_array()
            .expect("collected items"),
        &mut diagnostics,
    );
    super::emit(
        json!({"project": {
            "schema": "concord.project-observation/v1",
            "identity": project,
            "collections": collections,
            "complete": diagnostics.is_empty(),
            "diagnostics": diagnostics,
            "started_at": started,
            "observed_at": now(),
            "consistency": "live interval; Project metadata checked on every page and at completion; Issue facts can change independently; no atomic snapshot",
            "order": "items connection order is native Project item order; view sort is separately reported",
            "unknown": [
                "GraphQL view fields omit some native built-in fields, including Type; omission is not absence; visibleFields totalCount is not requested because its native resolver fails; its pageInfo remains authoritative",
                "workflow identity, name and enabled state are observable; trigger/action details and external automation are not exposed by this API",
                "Project membership is not delegation; viewerCanUpdate is provider capability only",
                "attached Issue values are read separately through the native Issue field endpoint; item.fieldValues omission is not an unset value"
            ],
            "semantics": "read-only observation; Issue body/comments own acceptance; no ranking inference, local planning replica or write authorization"
        }}),
        output,
    )
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn fault(code: &str, message: impl Into<String>) -> Error {
    Error::typed(format!("concord.project.{code}"), message)
}
