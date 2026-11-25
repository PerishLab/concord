use super::super::spawn::{self, Reply, Request, Server};
use serde_json::{Value, json};
use std::path::Path;
use std::process::Output;

struct Mock(String);

pub(super) fn meta() -> Value {
    let owner = json!({"__typename":"Organization", "id":"O_lab", "login":"PerishLab"});
    json!({"__typename":"ProjectV2", "id":"PVT_plan", "number":1, "title":"Planning", "url":"https://github.com/orgs/PerishLab/projects/1", "updatedAt":"2026-10-21T00:00:00Z", "closed":false, "public":false, "readme":"Boundary", "shortDescription":"Goal", "viewerCanUpdate":true, "owner":owner})
}

pub(super) fn overview(project: &Value) -> Value {
    let owner = json!({
        "kind": project["owner"]["__typename"],
        "id": project["owner"]["id"],
        "login": project["owner"]["login"],
    });
    json!({
        "id": project["id"],
        "number": project["number"],
        "title": project["title"],
        "url": project["url"],
        "updated": project["updatedAt"],
        "closed": project["closed"],
        "public": project["public"],
        "writable": project["viewerCanUpdate"],
        "description": project["shortDescription"],
        "readme": project["readme"],
        "owner": owner,
    })
}

pub(super) fn connection(nodes: Vec<Value>, more: bool) -> Value {
    let cursor = more.then_some("next");
    let page = json!({"hasNextPage":more, "endCursor":cursor});
    json!({"totalCount":nodes.len(), "nodes":nodes, "pageInfo":page})
}

fn field(id: &str, attached: bool) -> Value {
    let project = json!({"id":"PVT_plan"});
    let mut field = json!({"__typename":"ProjectV2SingleSelectField", "id":id, "name":"Priority", "dataType":"SINGLE_SELECT", "isIssueField":attached, "updatedAt":"2026-10-21T00:00:00Z", "project":project, "options":[]});
    if attached {
        field["issueField"] = json!({"id":"IFSS_priority", "fullDatabaseId":"42", "name":"Priority", "dataType":"SINGLE_SELECT"});
    }
    field
}

pub(super) fn item(kind: &str) -> Value {
    let empty = connection(vec![], false);
    let kind = json!({"id":"IT_kind", "name":kind});
    let repository = json!({"id":"R_repo", "nameWithOwner":"PerishLab/concord"});
    let content = json!({"__typename":"Issue", "id":"I_work", "number":4, "title":"Work", "url":"https://github.com/PerishLab/concord/issues/4", "state":"OPEN", "body":"## Acceptance\n- [ ] source", "updatedAt":"2026-10-21T00:00:00Z", "issueType":kind, "repository":repository, "parent":null, "blockedBy":empty, "blocking":empty, "subIssues":empty, "labels":empty, "comments":empty});
    json!({"id":"PVTI_work", "type":"ISSUE", "isArchived":false, "updatedAt":"2026-10-21T00:00:00Z", "project":{"id":"PVT_plan"}, "fieldValues":empty, "content":content})
}

pub(super) fn provider(mode: &str) -> Server {
    let mock = Mock(mode.to_owned());
    Server::start(move |request| mock.handle(request))
}

impl Mock {
    fn handle(&self, request: Request) -> Reply {
        if request.method == "GET" && request.path.contains("/issue-field-values") {
            let option = json!({"id": 5, "name": "High"});
            return Reply::json(
                json!([{"issue_field_id": 42, "data_type": "single_select", "node_id": "IFSSV_value", "value": 5, "single_select_option": option}]),
            );
        }
        if request.method != "POST" || request.path != "/graphql" {
            return Reply::status(404, json!({"message": "unexpected"}));
        }
        let query = request.query();
        if query.contains("mutation") {
            return Reply::status(403, json!({"message": "mutation forbidden"}));
        }
        let variables = request.variables();
        if query.contains("RunsealOverview") {
            return self.start();
        }
        if query.contains("RunsealSubject") {
            let node = json!({"id":"I_work","updated":"2026-10-21T00:00:00Z","repository":{"id":"R_repo"}});
            return Reply::json(json!({"data":{"node":node}}));
        }
        if query.contains("RunsealMetadata") {
            let mut finality = meta();
            if self.0 == "final" {
                finality["title"] = "Concurrent edit".into();
            }
            let node = overview(&finality);
            return Reply::json(json!({"data":{"node":node}}));
        }
        self.board(&query, &variables)
    }

    fn start(&self) -> Reply {
        let mut first = meta();
        if self.0 == "identity" {
            first["owner"]["login"] = "Foreign".into();
        }
        let organization = json!({"login":"PerishLab","projectV2":overview(&first)});
        let mut reply = json!({"data":{"organization":organization}});
        match self.0.as_str() {
            "permission" => reply["errors"] = json!([{"message":"FORBIDDEN"}]),
            "missing" => reply["data"]["organization"]["projectV2"] = Value::Null,
            _ => {}
        }
        Reply::json(reply)
    }

    fn board(&self, query: &str, variables: &Value) -> Reply {
        for name in [
            "fields",
            "items",
            "views",
            "workflows",
            "repositories",
            "statusUpdates",
        ] {
            if !query.contains(&format!("{name}(first:")) {
                continue;
            }
            let nodes = match name {
                "fields" => vec![field("PVTSSF_local", false), field("PVTSSF_attached", true)],
                "items" => vec![item(if self.0 == "auto" { "Auto" } else { "Feature" })],
                "views" => vec![view()],
                _ => vec![],
            };
            let mut root = meta();
            root[name] = connection(nodes, false);
            self.alter(name, &mut root);
            if let Some(continuation) = self.page(name, &mut root)
                && variables["after"].as_str() == Some("next")
            {
                let data = json!({"node":continuation});
                return Reply::json(json!({"data":data}));
            }
            let data = json!({"node":root});
            return Reply::json(json!({"data":data}));
        }
        Reply::status(400, json!({"message": "unknown query"}))
    }

    fn alter(&self, name: &str, root: &mut Value) {
        match (self.0.as_str(), name) {
            ("changed", "views") => root["title"] = "Concurrent edit".into(),
            ("schema", "fields") => root[name]["nodes"][0]["isIssueField"] = Value::Null,
            ("missing-connection", "items") => {
                root[name]["nodes"][0]["content"]
                    .as_object_mut()
                    .unwrap()
                    .remove("comments");
            }
            ("origin", "fields") => {
                root[name]["nodes"][1]["issueField"]["fullDatabaseId"] = Value::Null
            }
            ("builtin", "fields") => {
                root[name]["nodes"][0]["__typename"] = "ProjectV2Field".into();
                root[name]["nodes"][0]["dataType"] = "TITLE".into();
            }
            ("null", "items") => root[name]["nodes"][0]["content"] = Value::Null,
            ("nested", "items") => {
                root[name]["nodes"][0]["content"]["comments"] = connection(vec![], true);
                root[name]["nodes"][0]["content"]["comments"]["totalCount"] = 101.into();
            }
            _ => {}
        }
    }

    fn page(&self, name: &str, root: &mut Value) -> Option<Value> {
        if name != "fields" || !matches!(self.0.as_str(), "pages" | "repeat" | "cursor") {
            return None;
        }
        let mut continuation = root.clone();
        continuation[name]["nodes"] = json!([field("PVTSSF_attached", true)]);
        root[name]["nodes"] = json!([field("PVTSSF_local", false)]);
        root[name]["pageInfo"] = json!({"hasNextPage":true,"endCursor":"next"});
        match self.0.as_str() {
            "cursor" => root[name]["pageInfo"]["endCursor"] = Value::Null,
            "repeat" => continuation[name]["nodes"][0]["id"] = "PVTSSF_local".into(),
            _ => {}
        }
        Some(continuation)
    }
}

pub(super) fn run(root: &Path, mode: &str, pages: &str) -> Output {
    let server = provider(mode);
    let mut command = spawn::concord(root);
    server.attach(&mut command);
    command
        .current_dir(root)
        .args([
            "--home",
            "/deliberately-unavailable-estate",
            "--json",
            "project",
            "show",
            "https://github.com/orgs/PerishLab/projects/1",
            "--max-pages",
            pages,
        ])
        .output()
        .unwrap()
}

pub(super) fn decoded(output: &Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice::<Value>(&output.stdout).unwrap()["project"].clone()
}

fn view() -> Value {
    let empty = connection(vec![], false);
    let mut visible = empty.clone();
    visible.as_object_mut().unwrap().remove("totalCount");
    let config = json!({"visibleFields":visible});
    json!({"id":"PVTV_triage", "number":1, "name":"Triage", "layout":"TABLE_LAYOUT", "filter":"is:issue", "project":{"id":"PVT_plan"}, "configuration":config, "groupByFields":empty, "verticalGroupByFields":empty, "sortByFields":empty})
}
