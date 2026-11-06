use super::super::management::{Fixture, decoded, intent, refused};
use serde_json::{Value, json};

fn setup(mode: &str) -> Fixture {
    let fixture = Fixture::new(mode);
    fixture.change(|state| {
        state["fields"][0]["name"]=json!("Contribution");
        let item=json!({"id":"ITEM_one","type":"ISSUE","isArchived":false,"updatedAt":"original","project":{"id":"PVT_plan"},"content":state["issues"][0],"fieldValues":super::super::planning::connection(vec![],false)});
        state["items"]=json!([item]);
    });
    fixture
}

fn text(fixture: &Fixture) {
    fixture.change(|state| {
        state["fields"][0]["__typename"] = json!("ProjectV2Field");
        state["fields"][0]["dataType"] = json!("TEXT");
    });
}

fn action(selection: Value) -> Value {
    intent(json!({"kind":"value","field":"F_local","item":"ITEM_one","selection":selection}))
}

fn execute(fixture: &Fixture, selection: Value) -> Value {
    let plan = decoded(&fixture.run("prepare", &action(selection)));
    decoded(&fixture.run("apply", &plan))
}

#[test]
fn values() {
    let fixture = setup("ordinary");
    let issue = fixture.state()["issues"].clone();
    let result = execute(&fixture, json!({"kind":"select","option":"old"}));
    assert_eq!(result["project"]["result"]["value"]["optionId"], "old");
    assert!(execute(&fixture, json!({"kind":"clear"}))["project"]["result"]["value"].is_null());
    text(&fixture);
    let result = execute(&fixture, json!({"kind":"text","text":"原生审阅路径"}));
    assert_eq!(result["project"]["result"]["value"]["text"], "原生审阅路径");
    let writes = fixture.state()["writes"].clone();
    assert_eq!(
        execute(&fixture, json!({"kind":"text","text":"原生审阅路径"}))["project"]["outcome"],
        "unchanged"
    );
    assert_eq!(fixture.state()["writes"], writes);
    assert!(execute(&fixture, json!({"kind":"clear"}))["project"]["result"]["value"].is_null());
    assert_eq!(fixture.state()["issues"], issue);
}

#[test]
fn guards() {
    for name in ["Status", "Priority", "Cycle", "Phase", "Planning"] {
        let fixture = setup("ordinary");
        fixture.change(|state| state["fields"][0]["name"] = json!(name));
        refused(
            &fixture.run("prepare", &action(json!({"kind":"clear"}))),
            "concord.project.scope",
        );
        assert!(fixture.state()["writes"].as_array().unwrap().is_empty());
    }
    let fixture = setup("ordinary");
    refused(
        &fixture.run(
            "prepare",
            &action(json!({"kind":"select","option":"foreign"})),
        ),
        "concord.project.identity",
    );
    text(&fixture);
    refused(
        &fixture.run(
            "prepare",
            &action(json!({"kind":"text","text":"x".repeat(4097)})),
        ),
        "concord.project.bounds",
    );
    refused(
        &fixture.run("prepare", &action(json!({"kind":"text","text":"  "}))),
        "concord.project.intent",
    );
    for kind in ["Auto", "Unknown"] {
        fixture.change(|state| state["items"][0]["content"]["issueType"]["name"] = json!(kind));
        refused(
            &fixture.run("prepare", &action(json!({"kind":"clear"}))),
            "concord.project.custody",
        );
    }
}

#[test]
fn recovery() {
    let fixture = setup("lost-value");
    text(&fixture);
    let plan = decoded(&fixture.run("prepare", &action(json!({"kind":"text","text":"bounded"}))));
    refused(
        &fixture.run("apply", &plan),
        "concord.project.indeterminate",
    );
    let writes = fixture.state()["writes"].clone();
    assert_eq!(
        decoded(&fixture.run("recover", &plan))["project"]["desired_state"],
        "observed"
    );
    assert_eq!(fixture.state()["writes"], writes);
    let fixture = setup("ordinary");
    let plan = decoded(&fixture.run("prepare", &action(json!({"kind":"select","option":"old"}))));
    fixture.change(|state| state["items"][0]["content"]["updatedAt"] = json!("peer"));
    refused(&fixture.run("apply", &plan), "concord.project.stale");
    assert!(fixture.state()["writes"].as_array().unwrap().is_empty());
    let fixture = setup("delayed-value");
    assert_eq!(
        execute(&fixture, json!({"kind":"select","option":"old"}))["project"]["result"]["value"]["optionId"],
        "old"
    );
}

#[test]
fn independent() {
    let left = setup("ordinary");
    let right = setup("ordinary");
    text(&left);
    text(&right);
    right.change(|state| {
        state["project"]["id"] = json!("PVT_other");
        state["project"]["number"] = json!(2);
        state["project"]["url"] = json!("https://github.com/orgs/PerishLab/projects/2");
        for field in state["fields"].as_array_mut().unwrap() {
            field["project"]["id"] = json!("PVT_other");
        }
        state["fields"][0]["id"] = json!("F_other");
        state["items"][0]["project"]["id"] = json!("PVT_other");
    });
    execute(&left, json!({"kind":"text","text":"Product benefit"}));
    assert_eq!(right.state()["items"][0]["fieldValues"]["totalCount"], 0);
    let mut request = action(json!({"kind":"text","text":"Infrastructure benefit"}));
    request["action"]["field"] = json!("F_other");
    let plan = decoded(&right.target("prepare", "PerishLab/2", &request));
    decoded(&right.target("apply", "PerishLab/2", &plan));
    assert_eq!(
        left.state()["items"][0]["fieldValues"]["nodes"][0]["text"],
        "Product benefit"
    );
    assert_eq!(
        right.state()["items"][0]["fieldValues"]["nodes"][0]["text"],
        "Infrastructure benefit"
    );
    assert_eq!(left.state()["issues"], right.state()["issues"]);
}

#[test]
fn qualification() {
    for mode in [
        "attached",
        "foreign-item",
        "foreign-field",
        "workflow",
        "closed",
        "type",
        "duplicate",
    ] {
        let fixture = setup("ordinary");
        fixture.change(|state| match mode {
            "attached" => attach(state),
            "foreign-item" => state["items"][0]["project"]["id"]=json!("foreign"),
            "foreign-field" => state["fields"][0]["project"]["id"]=json!("foreign"),
            "workflow" => state["workflows"]=workflow(),
            "closed" => state["items"][0]["content"]["state"]=json!("CLOSED"),
            "type" => state["fields"][0]["options"]=json!([{"id":"a","name":"Urgent"},{"id":"b","name":"High"},{"id":"c","name":"Medium"},{"id":"d","name":"Low"}]),
            "duplicate" => state["fields"][0]["options"]=json!([{"id":"old","name":"a"},{"id":"old","name":"b"}]),
            _ => unreachable!(),
        });
        let output = fixture.run("prepare", &action(json!({"kind":"clear"})));
        assert!(!output.status.success(), "{mode}");
        assert!(fixture.state()["writes"].as_array().unwrap().is_empty());
    }
    let fixture = setup("ordinary");
    assert!(
        !fixture
            .run("prepare", &action(json!({"kind":"text"})))
            .status
            .success()
    );
    assert!(fixture.state()["writes"].as_array().unwrap().is_empty());
}

fn attach(state: &mut Value) {
    state["fields"][0] = state["fields"][1].clone();
    state["fields"][0]["id"] = json!("F_local");
}

fn workflow() -> Value {
    json!([{"id":"W","name":"Auto","enabled":true,"updatedAt":"old","project":{"id":"PVT_plan"}}])
}
