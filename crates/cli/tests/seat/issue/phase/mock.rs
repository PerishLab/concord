use super::world::page;
use crate::spawn::{Reply, Request};
use serde_json::{Value, json};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

pub(super) fn serve(shared: &Mutex<Value>, request: Request) -> Reply {
    let mut state = shared.lock().unwrap();
    let mode = state["mode"].as_str().unwrap_or("ordinary").to_owned();
    if request.method == "GET" {
        record(&mut state, "peer");
        if matches!(mode.as_str(), "unavailable" | "peer") {
            return Reply::status(500, json!({"message": "unavailable"}));
        }
        return Reply::json(json!([]));
    }
    let query = request.query();
    let kind = if query.contains("RunsealIssueFields") {
        "schema"
    } else if query.contains("RunsealPhase") {
        "values"
    } else {
        "root"
    };
    record(&mut state, kind);
    if mode == "unavailable" || (mode == "provider" && kind == "root") {
        return Reply::status(500, json!({"message": "unavailable"}));
    }
    if query.starts_with("mutation") {
        return mutate(&mut state, &mode, request);
    }
    if kind == "schema" {
        return schema(&state);
    }
    if query.contains("RunsealOverview") {
        let org = json!({"login":"PerishLab","projectV2":overview()});
        return Reply::json(json!({"data":{"organization":org}}));
    }
    if query.contains("RunsealMetadata") {
        return Reply::json(json!({"data":{"node":overview()}}));
    }
    if query.contains("RunsealBoard") {
        return board(&state["issue"], &query);
    }
    if query.contains("RunsealConfirm") {
        return Reply::json(json!({"data":{"node":state["issue"]}}));
    }
    read(&state["issue"], &mode, kind, &query)
}

fn record(state: &mut Value, kind: &str) {
    state["reads"].as_array_mut().unwrap().push(json!(kind));
}

fn mutate(state: &mut Value, mode: &str, request: Request) -> Reply {
    let body = request.json();
    let fields = body["variables"]["input"]["issueFields"]
        .as_array()
        .unwrap();
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0]["fieldId"], "IFSS_phase");
    state["writes"].as_array_mut().unwrap().push(body.clone());
    let mut values = state["issue"]["issueFieldValues"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|entry| entry["field"]["id"] != "IFSS_phase")
        .cloned()
        .collect::<Vec<_>>();
    if !fields[0]["delete"].as_bool().unwrap_or(false) {
        let wanted = fields[0]["singleSelectOptionId"].as_str().unwrap();
        let option = state["field"]["options"]
            .as_array()
            .unwrap()
            .iter()
            .find(|option| option["id"] == wanted)
            .unwrap();
        values.push(json!({
            "__typename":"IssueFieldSingleSelectValue","id":"V_phase",
            "optionId":option["id"],"name":option["name"],"field":{"id":"IFSS_phase"}
        }));
    }
    state["issue"]["issueFieldValues"] = page(values);
    match mode {
        "stamp" => state["issue"]["updatedAt"] = "phase-only".into(),
        "foreign" => state["issue"]["body"] = "Changed by another actor".into(),
        "unknown" => return Reply::status(500, json!({"message": "unknown"})),
        _ => {}
    }
    let change = json!({"issue":{"id":state["issue"]["id"]}});
    Reply::json(json!({"data":{"change":change}}))
}

fn schema(state: &Value) -> Reply {
    let field = &state["field"];
    let row = json!({
        "kind": field["__typename"], "id": field["id"], "name": field["name"],
        "datatype": field["dataType"], "visibility": field["visibility"],
        "options": field["options"],
    });
    let list = json!({"total": 1, "rows": [row], "page": {"more": false, "cursor": null}});
    let org = json!({"id": state["owner"], "login": "PerishLab", "list": list});
    Reply::json(json!({"data": {"organization": org}}))
}

fn read(source: &Value, mode: &str, kind: &str, query: &str) -> Reply {
    let mut issue = source.clone();
    match (mode, kind) {
        ("drift", "values") => issue["id"] = "I_changed".into(),
        ("paging", "root") => {
            issue["blocking"]["pageInfo"] = json!({"hasNextPage":true,"endCursor":null});
        }
        ("minimal", "values") => issue = minimal(&issue, query),
        _ => {}
    }
    let repo = json!({"id":"R_repo","nameWithOwner":"PerishLab/concord","issue":issue});
    Reply::json(json!({"data":{"repository":repo}}))
}

fn minimal(issue: &Value, query: &str) -> Value {
    let forbidden = [
        "body",
        "state",
        "updatedAt",
        "viewerCanUpdate",
        "parent",
        "subIssues",
        "labels",
        "blockedBy",
        "blocking",
        "comments",
        "issueType",
    ];
    for word in forbidden {
        assert!(!query.contains(word), "{word}");
    }
    json!({
        "id": issue["id"], "number": issue["number"], "url": issue["url"],
        "repository": issue["repository"], "issueFieldValues": issue["issueFieldValues"],
    })
}

fn overview() -> Value {
    let owner = json!({"kind": "Organization", "id": "O_lab", "login": "PerishLab"});
    json!({
        "id": "PVT_plan", "number": 1, "title": "Plan",
        "url": "https://github.com/orgs/PerishLab/projects/1", "updated": "original",
        "closed": false, "public": false, "writable": true,
        "description": "Goal", "readme": "Boundary", "owner": owner,
    })
}

fn board(issue: &Value, query: &str) -> Reply {
    let owner = json!({"__typename": "Organization", "id": "O_lab", "login": "PerishLab"});
    let mut project = json!({
        "__typename": "ProjectV2", "id": "PVT_plan", "number": 1, "title": "Plan",
        "url": "https://github.com/orgs/PerishLab/projects/1", "updatedAt": "original",
        "closed": false, "public": false, "readme": "Boundary",
        "shortDescription": "Goal", "viewerCanUpdate": true, "owner": owner,
    });
    if query.contains("fields(first:") {
        let iteration = json!({"id":"C_now","title":"Now","startDate":today(),"duration":7});
        let config =
            json!({"duration":7,"startDay":1,"iterations":[iteration],"completedIterations":[]});
        let cycle = json!({
            "__typename":"ProjectV2IterationField","id":"F_cycle","name":"Cycle",
            "dataType":"ITERATION","isIssueField":false,"project":{"id":"PVT_plan"},
            "updatedAt":"original","configuration":config
        });
        let options = ["Urgent", "High", "Medium", "Low"]
            .into_iter()
            .enumerate()
            .map(|(index, name)| json!({"id": index.to_string(), "name": name}))
            .collect::<Vec<_>>();
        let priority = json!({
            "__typename":"ProjectV2SingleSelectField","id":"F_priority","name":"Priority",
            "dataType":"SINGLE_SELECT","isIssueField":false,"project":{"id":"PVT_plan"},
            "updatedAt":"original","options":options
        });
        project["fields"] = page(vec![priority, cycle]);
    }
    if query.contains("items(first:") {
        let cell = json!({"__typename":"ProjectV2ItemFieldIterationValue","field":{"id":"F_cycle"},"iterationId":"C_now"});
        let first = json!({
            "id":"ITEM_first","type":"ISSUE","isArchived":false,
            "project":{"id":"PVT_plan"},"updatedAt":"original",
            "content":issue,"fieldValues":page(vec![cell])
        });
        let mut second = first.clone();
        second["id"] = "ITEM_second".into();
        second["content"]["id"] = "I_second".into();
        project["items"] = page(vec![first, second]);
    }
    Reply::json(json!({"data":{"node":project}}))
}

fn today() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
        + 8 * 3600;
    let days = secs.div_euclid(86_400) + 719_468;
    let era = days.div_euclid(146_097);
    let offset = (days - era * 146_097) as u64;
    let years = (offset - offset / 1460 + offset / 36_524 - offset / 146_096) / 365;
    let year = years as i64 + era * 400;
    let ordinal = offset - (365 * years + years / 4 - years / 100);
    let month = (5 * ordinal + 2) / 153;
    let day = ordinal - (153 * month + 2) / 5 + 1;
    let month = if month < 10 { month + 3 } else { month - 9 };
    let year = year + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}
