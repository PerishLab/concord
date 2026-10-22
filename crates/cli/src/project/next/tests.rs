use super::{super::planning::Date, select};
use serde_json::{Value, json};

fn connection(nodes: Vec<Value>) -> Value {
    json!({"totalCount":nodes.len(),"nodes":nodes,"pageInfo":{"hasNextPage":false,"endCursor":null}})
}

fn field(id: &str, kind: &str) -> Value {
    json!({"id":id,"authority":"project","dataType":kind})
}

fn item(id: &str, level: &str) -> Value {
    let repository = json!({"owner":{"id":"O_lab"}});
    json!({"id":id,"isArchived":false,"fieldValues":connection(vec![
        json!({"__typename":"ProjectV2ItemFieldIterationValue","field":{"id":"cycle"},"iterationId":"c1"}),
        json!({"__typename":"ProjectV2ItemFieldSingleSelectValue","field":{"id":"priority"},"optionId":level})
    ]),"content":{"__typename":"Issue","id":format!("I_{id}"),"title":id,"url":format!("https://github.com/PerishLab/demo/issues/{id}"),"updatedAt":"2026-10-22T00:00:00Z","state":"OPEN","issueType":{"name":"Task"},"repository":repository,"labels":connection(vec![]),"blockedBy":connection(vec![])}})
}

fn state() -> Value {
    let mut cycle = field("cycle", "ITERATION");
    cycle["configuration"] = json!({"iterations":[{"id":"c1","title":"Current","startDate":"2026-10-21","duration":7}],"completedIterations":[]});
    let mut priority = field("priority", "SINGLE_SELECT");
    priority["options"] = json!([{"id":"u","name":"Urgent"},{"id":"h","name":"High"},{"id":"m","name":"Medium"},{"id":"l","name":"Low"}]);
    json!({"project":{"id":"PVT_plan","closed":false,"owner":{"id":"O_lab"}},"fields":[cycle,priority],"items":[item("first","l"),item("second","h")]})
}

fn day(date: &str) -> i64 {
    Date::parse(date).unwrap().day()
}

#[test]
fn ordered() {
    let state = state();
    let next = select(&state, day("2026-10-22")).unwrap();
    assert_eq!(next["item"], "first");
    assert_eq!(next["priority"]["name"], "Low");
    assert_eq!(next["cycle"]["id"], "c1");
}

#[test]
fn skipped() {
    for mode in [
        "closed",
        "archived",
        "auto",
        "unknown",
        "needs",
        "blocked",
        "unassigned",
        "future",
    ] {
        let mut state = state();
        let first = &mut state["items"][0];
        let needs = connection(vec![json!({"id":"L","name":"needs:revalidation"})]);
        let blocked = connection(vec![json!({"id":"I_block","state":"OPEN"})]);
        match mode {
            "closed" => first["content"]["state"] = json!("CLOSED"),
            "archived" => first["isArchived"] = json!(true),
            "auto" => first["content"]["issueType"]["name"] = json!("Auto"),
            "unknown" => first["content"]["issueType"] = Value::Null,
            "needs" => first["content"]["labels"] = needs,
            "blocked" => first["content"]["blockedBy"] = blocked,
            "unassigned" => first["fieldValues"] = connection(vec![]),
            "future" => first["fieldValues"]["nodes"][0]["iterationId"] = json!("c2"),
            _ => unreachable!(),
        }
        assert_eq!(
            select(&state, day("2026-10-22")).unwrap()["item"],
            "second",
            "{mode}"
        );
    }
}

#[test]
fn observations() {
    let mut state = state();
    let first = &mut state["items"][0];
    first["content"]["blockedBy"] = connection(vec![json!({"id":"I_closed","state":"CLOSED"})]);
    first["content"]["subIssues"] = connection(vec![json!({"id":"I_child","state":"OPEN"})]);
    first["content"]["assignees"] = connection(vec![json!({"login":"another-agent"})]);
    first["fieldValues"]["nodes"]
        .as_array_mut()
        .unwrap()
        .remove(1);
    first["fieldValues"]["totalCount"] = json!(1);
    let next = select(&state, day("2026-10-22")).unwrap();
    assert_eq!(next["item"], "first");
    assert!(next["priority"].is_null());
}

#[test]
fn calendar() {
    assert_eq!(day("1970-01-01"), 0);
    assert_eq!(day("1970-01-02"), 1);
    assert_eq!(day("1969-12-31"), -1);
    assert_eq!(day("2000-03-01") - day("2000-02-28"), 2);
    assert_eq!(day("2100-03-01") - day("2100-02-28"), 1);
    let state = state();
    assert!(select(&state, day("2026-10-20")).unwrap()["issue"].is_null());
    assert_eq!(select(&state, day("2026-10-21")).unwrap()["item"], "first");
    assert_eq!(select(&state, day("2026-10-27")).unwrap()["item"], "first");
    assert!(select(&state, day("2026-10-28")).unwrap()["issue"].is_null());
}

#[test]
fn abstention() {
    for mode in [
        "closed",
        "fields",
        "attached",
        "ambiguous-field",
        "overlap",
        "empty",
    ] {
        let mut state = state();
        match mode {
            "closed" => state["project"]["closed"] = json!(true),
            "fields" => state["fields"] = json!([]),
            "attached" => state["fields"][1]["authority"] = json!("issue"),
            "ambiguous-field" => {
                let mut duplicate = state["fields"][0].clone();
                duplicate["id"] = json!("another-cycle");
                state["fields"].as_array_mut().unwrap().push(duplicate);
            }
            "overlap" => {
                let mut duplicate = state["fields"][0]["configuration"]["iterations"][0].clone();
                duplicate["id"] = json!("c2");
                state["fields"][0]["configuration"]["iterations"]
                    .as_array_mut()
                    .unwrap()
                    .push(duplicate);
            }
            "empty" => state["items"] = json!([]),
            _ => unreachable!(),
        }
        assert!(
            select(&state, day("2026-10-22")).unwrap()["issue"].is_null(),
            "{mode}"
        );
    }
}

#[test]
fn incomplete() {
    for mode in [
        "labels",
        "blockers",
        "values",
        "null",
        "state",
        "owner",
        "priority",
        "duplicate",
    ] {
        let mut state = state();
        let first = &mut state["items"][0];
        match mode {
            "labels" => first["content"]["labels"]["pageInfo"]["hasNextPage"] = json!(true),
            "blockers" => first["content"]["blockedBy"]["totalCount"] = json!(1),
            "values" => first["fieldValues"]["pageInfo"]["hasNextPage"] = json!(true),
            "null" => first["content"]["labels"] = connection(vec![Value::Null]),
            "state" => first["content"]["blockedBy"] = connection(vec![json!({"id":"I_block"})]),
            "owner" => first["content"]["repository"]["owner"]["id"] = json!("O_foreign"),
            "priority" => first["fieldValues"]["nodes"][1]["optionId"] = json!("foreign"),
            "duplicate" => {
                let duplicate = first["fieldValues"]["nodes"][0].clone();
                first["fieldValues"]["nodes"]
                    .as_array_mut()
                    .unwrap()
                    .push(duplicate);
                first["fieldValues"]["totalCount"] = json!(3);
            }
            _ => unreachable!(),
        }
        assert!(select(&state, day("2026-10-22")).is_err(), "{mode}");
    }
}
