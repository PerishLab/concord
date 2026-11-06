use super::super::super::{
    fault, inspect,
    next::scope::Scope,
    planning, query,
    target::Target,
    wire::{self, Client, Spec},
};
use concord_core::Result;
use serde_json::{Value, json};

const ITEM: &str = "id type isArchived updatedAt project{id} content{__typename ... on Issue{id number title url state updatedAt issueType{id name} repository{id nameWithOwner owner{id login}}}}";

pub(super) async fn project(
    client: &Client<'_>,
    target: &Target,
    context: (&Value, i64),
    pages: u16,
) -> Result<Value> {
    let statement = format!(
        "query($owner:String!,$number:Int!){{organization(login:$owner){{projectV2(number:$number){{{}}}}}}}",
        query::META
    );
    let data = client
        .read(
            &statement,
            json!({"owner":target.owner,"number":target.number}),
        )
        .await?;
    let project = &data["organization"]["projectV2"];
    target.verify(project)?;
    if project["owner"]["id"] != context.0["id"] || !project["closed"].is_boolean() {
        return Err(fault(
            "identity",
            "Project owner or lifecycle disagrees with inventory",
        ));
    }
    let mut fields = wire::gather(client, project, "fields", (100, pages)).await?;
    inspect::fields(&mut fields.nodes, project)?;
    let spec = Spec {
        name: "items",
        query: connection(),
        size: 20,
        pages,
    };
    let items = wire::collect(client, project, spec).await?;
    inspect::membership(&items.nodes, project)?;
    let known = contents(&items.nodes, context.0)?;
    let mut diagnostics = Vec::new();
    inspect::gaps(&items.value(), "items", &mut diagnostics)?;
    let membership = items.complete && known && diagnostics.is_empty();
    let complete = fields.complete && membership;
    let planning = if !fields.complete {
        json!({"status":"incomplete","cycle":null})
    } else if project["closed"] == true {
        json!({"status":"closed","cycle":null})
    } else {
        let definition = json!(fields.nodes);
        let (scope, status) = Scope::inspect(&definition, context.1)?;
        json!({"status":status,"cycle":scope.map(|scope|scope.period.clone())})
    };
    Ok(
        json!({"project":project,"fields":fields.value(),"items":items.value(),"complete":complete,"planning":planning,"diagnostics":diagnostics,"content_complete":known}),
    )
}

pub(super) async fn finish(client: &Client<'_>, project: &Value) -> Result<()> {
    let statement = format!(
        "query($id:ID!){{node(id:$id){{... on ProjectV2{{{}}}}}}}",
        query::META
    );
    let data = client.read(&statement, json!({"id":project["id"]})).await?;
    if data["node"] != *project {
        return Err(fault(
            "changed",
            "Project metadata changed before survey completed",
        ));
    }
    Ok(())
}

fn contents(items: &[Value], owner: &Value) -> Result<bool> {
    let mut complete = true;
    for item in items {
        if !item["isArchived"].is_boolean() {
            return Err(fault("schema", "item archival state is unknown"));
        }
        let issue = &item["content"];
        match issue["__typename"].as_str() {
            Some("Issue") => identity(issue, owner)?,
            Some("PullRequest" | "DraftIssue") => {}
            _ => complete = false,
        }
    }
    Ok(complete)
}

fn identity(issue: &Value, owner: &Value) -> Result<()> {
    let repo = &issue["repository"];
    let number = issue["number"].as_u64().filter(|number| *number > 0);
    let name = repo["nameWithOwner"].as_str().unwrap_or("");
    let prefix = format!("{}/", owner["login"].as_str().unwrap_or(""));
    if issue["id"].as_str().is_none_or(str::is_empty)
        || repo["id"].as_str().is_none_or(str::is_empty)
        || repo["owner"]["id"] != owner["id"]
    {
        return Err(fault(
            "identity",
            "Project Issue is missing or outside the organization scope",
        ));
    }
    if !name.starts_with(&prefix)
        || number.is_none()
        || issue["url"] != format!("https://github.com/{name}/issues/{}", number.unwrap_or(0))
    {
        return Err(fault(
            "identity",
            "Project Issue coordinate disagrees with its scope",
        ));
    }
    if !matches!(issue["state"].as_str(), Some("OPEN" | "CLOSED"))
        || !issue["title"].is_string()
        || issue["updatedAt"].as_str().is_none_or(str::is_empty)
    {
        return Err(fault("schema", "Project Issue lifecycle is unreadable"));
    }
    Ok(())
}

fn connection() -> String {
    let selection = planning::selection(ITEM).replace(
        "ProjectV2ItemFieldTextValue{field",
        "ProjectV2ItemFieldTextValue{text field",
    );
    "query($id:ID!,$size:Int!,$after:String){node(id:$id){... on ProjectV2{$META items(first:$size,after:$after){$PAGE nodes{$ITEM}}}}}"
        .replace("$META", query::META)
        .replace("$PAGE", query::PAGE)
        .replace("$ITEM", &selection)
}
