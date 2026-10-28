use super::super::spawn;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::Output;

pub(super) fn meta() -> Value {
    json!({"__typename":"ProjectV2", "id":"PVT_plan", "number":1, "title":"Planning", "url":"https://github.com/orgs/PerishLab/projects/1", "updatedAt":"2026-10-21T00:00:00Z", "closed":false, "public":false, "readme":"Boundary", "shortDescription":"Goal", "viewerCanUpdate":true, "owner":{"__typename":"Organization", "id":"O_lab", "login":"PerishLab"}})
}

pub(super) fn connection(nodes: Vec<Value>, more: bool) -> Value {
    json!({"totalCount":nodes.len(), "nodes":nodes, "pageInfo":{"hasNextPage":more, "endCursor":if more {Some("next")} else {None}}})
}

fn field(id: &str, attached: bool) -> Value {
    let mut field = json!({"__typename":"ProjectV2SingleSelectField", "id":id, "name":"Priority", "dataType":"SINGLE_SELECT", "isIssueField":attached, "updatedAt":"2026-10-21T00:00:00Z", "project":{"id":"PVT_plan"}, "options":[]});
    if attached {
        field["issueField"] = json!({"id":"IFSS_priority", "fullDatabaseId":"42", "name":"Priority", "dataType":"SINGLE_SELECT"});
    }
    field
}

pub(super) fn item(kind: &str) -> Value {
    let empty = connection(vec![], false);
    json!({"id":"PVTI_work", "type":"ISSUE", "isArchived":false, "updatedAt":"2026-10-21T00:00:00Z", "project":{"id":"PVT_plan"}, "fieldValues":empty, "content":{"__typename":"Issue", "id":"I_work", "number":4, "title":"Work", "url":"https://github.com/PerishLab/concord/issues/4", "state":"OPEN", "body":"## Acceptance\n- [ ] source", "updatedAt":"2026-10-21T00:00:00Z", "issueType":{"id":"IT_kind", "name":kind}, "repository":{"id":"R_repo", "nameWithOwner":"PerishLab/concord"}, "parent":null, "blockedBy":empty, "blocking":empty, "subIssues":empty, "labels":empty, "comments":empty}})
}

pub(super) fn provider(root: &Path, mode: &str) -> PathBuf {
    let project = meta();
    let mut script = String::from(
        "#!/bin/sh\ncase \"$*\" in\n'api graphql --input -') ;;\n'api --method GET repos/'*) printf '%s\\n' '[{\"issue_field_id\":42,\"data_type\":\"single_select\",\"node_id\":\"IFSSV_value\",\"value\":5,\"single_select_option\":{\"id\":5,\"name\":\"High\"}}]'; exit 0 ;;\n*) exit 9 ;;\nesac\ninput=$(cat)\ncase \"$input\" in *mutation*) exit 8;; esac\ncase \"$input\" in\n",
    );
    let mut first = project.clone();
    if mode == "identity" {
        first["owner"]["login"] = "Foreign".into();
    }
    let mut start = json!({"data":{"organization":{"projectV2":first}}});
    match mode {
        "permission" => start["errors"] = json!([{"message":"FORBIDDEN"}]),
        "missing" => start["data"]["organization"]["projectV2"] = Value::Null,
        _ => {}
    }
    script.push_str(&format!("*'organization(login:'*) {} ;;\n", reply(&start)));
    for name in [
        "fields",
        "items",
        "views",
        "workflows",
        "repositories",
        "statusUpdates",
    ] {
        let nodes = match name {
            "fields" => vec![field("PVTSSF_local", false), field("PVTSSF_attached", true)],
            "items" => vec![item(if mode == "auto" { "Auto" } else { "Feature" })],
            "views" => vec![view()],
            _ => vec![],
        };
        let mut root = project.clone();
        root[name] = connection(nodes, false);
        alter(mode, name, &mut root);
        if let Some(continuation) = page(mode, name, &mut root) {
            script.push_str(&format!(
                "*'fields(first:'*'\"after\":\"next\"'*) {} ;;\n",
                node(&continuation)
            ));
        }
        script.push_str(&format!("*'{name}(first:'*) {} ;;\n", node(&root)));
    }
    let repository = json!({"id":"R_repo"});
    let stamp = json!({"id":"I_work", "updatedAt":"2026-10-21T00:00:00Z", "repository":repository});
    script.push_str(&format!("*'on Issue'*) {} ;;\n", node(&stamp)));
    let mut finality = project.clone();
    if mode == "final" {
        finality["title"] = "Concurrent edit".into();
    }
    script.push_str(&format!("*) {} ;;\nesac\n", node(&finality)));
    let path = root.join(format!("provider-{mode}"));
    plumb::testing::executable(&path, script);
    path
}

fn reply(value: &Value) -> String {
    let encoded = value.to_string().replace('\'', "'\\''");
    format!("printf '%s\\n' '{encoded}'")
}

pub(super) fn run(root: &Path, mode: &str, pages: &str) -> Output {
    let command = provider(root, mode);
    spawn::concord(root)
        .current_dir(root)
        .args([
            "--root",
            "/deliberately-unavailable-estate",
            "--json",
            "project",
            "show",
            "https://github.com/orgs/PerishLab/projects/1",
            "--github-command",
        ])
        .arg(command)
        .args(["--max-pages", pages])
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

fn alter(mode: &str, name: &str, root: &mut Value) {
    match (mode, name) {
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

fn page(mode: &str, name: &str, root: &mut Value) -> Option<Value> {
    if name != "fields" || !matches!(mode, "pages" | "repeat" | "cursor") {
        return None;
    }
    let mut continuation = root.clone();
    continuation[name]["nodes"] = json!([field("PVTSSF_attached", true)]);
    root[name]["nodes"] = json!([field("PVTSSF_local", false)]);
    root[name]["pageInfo"] = json!({"hasNextPage":true,"endCursor":"next"});
    match mode {
        "cursor" => root[name]["pageInfo"]["endCursor"] = Value::Null,
        "repeat" => continuation[name]["nodes"][0]["id"] = "PVTSSF_local".into(),
        _ => {}
    }
    Some(continuation)
}

fn node(value: &Value) -> String {
    reply(&json!({"data":{"node":value}}))
}

fn view() -> Value {
    let empty = connection(vec![], false);
    let mut visible = empty.clone();
    visible.as_object_mut().unwrap().remove("totalCount");
    json!({"id":"PVTV_triage", "number":1, "name":"Triage", "layout":"TABLE_LAYOUT", "filter":"is:issue", "project":{"id":"PVT_plan"}, "configuration":{"visibleFields":visible}, "groupByFields":empty, "verticalGroupByFields":empty, "sortByFields":empty})
}
