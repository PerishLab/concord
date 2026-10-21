use super::management::{Fixture, decoded, intent, refused};
use serde_json::{Value, json};

fn setup() -> Fixture {
    let fixture = Fixture::new("ordinary");
    fixture.change(|state| {
        for index in [0, 1] {
            state["fields"][index]["name"] = "Priority".into();
            state["fields"][index]["options"] = options();
        }
        let first = item("ITEM_one", &state["issues"][0]);
        let second = item("ITEM_two", &state["issues"][1]);
        state["items"] = json!([first, second]);
    });
    fixture
}

fn options() -> Value {
    json!([{"id":"p_urgent","name":"Urgent","color":"RED","description":""},{"id":"p_high","name":"High","color":"ORANGE","description":""},{"id":"p_medium","name":"Medium","color":"YELLOW","description":""},{"id":"p_low","name":"Low","color":"BLUE","description":""}])
}

fn item(id: &str, issue: &Value) -> Value {
    let values = super::planning::connection(vec![], false);
    json!({"id":id,"type":"ISSUE","isArchived":false,"updatedAt":"original","project":{"id":"PVT_plan"},"content":issue,"fieldValues":values})
}

fn change(value: Value) -> Value {
    intent(json!({"kind":"planning","change":value}))
}

fn execute(fixture: &Fixture, action: Value) -> Value {
    let plan = decoded(&fixture.run("prepare", &change(action)));
    decoded(&fixture.run("apply", &plan))
}

fn definition() -> Value {
    let current = period("period_now", "Current", "2026-10-21");
    let next = period("period_next", "Future", "2026-10-28");
    let past = period("period_old", "Past", "2026-10-14");
    json!({"__typename":"ProjectV2IterationField","id":"F_cycle","name":"Cycle","dataType":"ITERATION","isIssueField":false,"updatedAt":"original","project":{"id":"PVT_plan"},"configuration":{"duration":7,"startDay":3,"iterations":[current,next],"completedIterations":[past]}})
}

fn period(id: &str, title: &str, start: &str) -> Value {
    json!({"id":id,"title":title,"startDate":start,"duration":7})
}

fn cycles(fixture: &Fixture) {
    fixture.change(|state| state["fields"].as_array_mut().unwrap().push(definition()));
}

#[test]
fn priority() {
    let fixture = setup();
    let action = json!({"kind":"priority","field":"F_local","item":"ITEM_one","level":"High"});
    let result = execute(&fixture, action);
    assert_eq!(result["project"]["result"]["value"]["optionId"], "p_high");
    let plan = decoded(&fixture.run(
        "prepare",
        &change(json!({"kind":"priority","field":"F_local","item":"ITEM_one","level":null})),
    ));
    let result = decoded(&fixture.run("apply", &plan));
    assert!(result["project"]["result"]["value"].is_null());
    let writes = fixture.state()["writes"].clone();
    let plan = decoded(&fixture.run(
        "prepare",
        &change(json!({"kind":"priority","field":"F_local","item":"ITEM_one","level":null})),
    ));
    assert_eq!(
        decoded(&fixture.run("apply", &plan))["project"]["outcome"],
        "unchanged"
    );
    assert_eq!(fixture.state()["writes"], writes);
    assert_eq!(fixture.state()["issues"][0]["state"], "OPEN");
}

#[test]
fn ownership() {
    let fixture = setup();
    refused(
        &fixture.run(
            "prepare",
            &change(json!({"kind":"priority","field":"F_issue","item":"ITEM_one","level":"High"})),
        ),
        "concord.project.ownership",
    );
    for mode in ["Auto", "unknown", "foreign"] {
        fixture.change(|state| match mode {
            "Auto" => state["items"][0]["content"]["issueType"]["name"] = "Auto".into(),
            "unknown" => state["items"][0]["content"]["issueType"] = Value::Null,
            "foreign" => state["items"][0]["project"]["id"] = "PVT_foreign".into(),
            _ => unreachable!(),
        });
        let code = if mode == "foreign" {
            "concord.project.identity"
        } else {
            "concord.project.custody"
        };
        refused(
            &fixture.run(
                "prepare",
                &change(
                    json!({"kind":"priority","field":"F_local","item":"ITEM_one","level":"Low"}),
                ),
            ),
            code,
        );
    }
    assert!(fixture.state()["writes"].as_array().unwrap().is_empty());
}

#[test]
fn assignment() {
    let fixture = setup();
    cycles(&fixture);
    let set = json!({"kind":"assign","field":"F_cycle","item":"ITEM_one","period":"period_now"});
    assert_eq!(
        execute(&fixture, set)["project"]["result"]["value"]["iterationId"],
        "period_now"
    );
    let clear = json!({"kind":"assign","field":"F_cycle","item":"ITEM_one","period":null});
    assert!(execute(&fixture, clear)["project"]["result"]["value"].is_null());
    refused(&fixture.run("prepare",&change(json!({"kind":"assign","field":"F_cycle","item":"ITEM_one","period":"foreign-period"}))),"concord.project.period");
}

#[test]
fn ordering() {
    let fixture = setup();
    cycles(&fixture);
    for item in ["ITEM_one", "ITEM_two"] {
        execute(
            &fixture,
            json!({"kind":"assign","field":"F_cycle","item":item,"period":"period_now"}),
        );
    }
    let result = execute(
        &fixture,
        json!({"kind":"order","field":"F_cycle","period":"period_now","item":"ITEM_one","after":"ITEM_two"}),
    );
    assert_eq!(
        result["project"]["result"]["ordered_items"],
        json!(["ITEM_two", "ITEM_one"])
    );
    execute(
        &fixture,
        json!({"kind":"assign","field":"F_cycle","item":"ITEM_two","period":"period_next"}),
    );
    refused(&fixture.run("prepare",&change(json!({"kind":"order","field":"F_cycle","period":"period_now","item":"ITEM_one","after":"ITEM_two"}))),"concord.project.period");
}

#[test]
fn history() {
    let fixture = setup();
    cycles(&fixture);
    let mut periods = json!([{"id":"period_old","title":"Past","start":"2026-10-14","duration":7},{"id":"period_now","title":"Current","start":"2026-10-21","duration":7},{"id":"period_next","title":"Future","start":"2026-10-28","duration":7}]);
    let update = json!({"kind":"cycle","field":"F_cycle","name":"Cycle","start":"2026-10-14","duration":7,"periods":periods});
    let mut missing = update.clone();
    missing["periods"].as_array_mut().unwrap().remove(0);
    missing["start"] = "2026-10-21".into();
    refused(
        &fixture.run("prepare", &change(missing)),
        "concord.project.history",
    );
    periods
        .as_array_mut()
        .unwrap()
        .push(json!({"title":"Following","start":"2026-11-04","duration":7}));
    let mut update = update;
    update["periods"] = periods;
    let result = execute(&fixture, update);
    assert_eq!(
        result["project"]["result"]["configuration"]["completedIterations"][0]["id"],
        "period_old"
    );
    assert_eq!(
        result["project"]["result"]["configuration"]["iterations"][0]["id"],
        "period_now"
    );
    assert_eq!(
        result["project"]["result"]["configuration"]["iterations"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
}

#[test]
fn creation() {
    let fixture = setup();
    let create = json!({"kind":"cycle","field":null,"name":"Cycle","start":"2026-10-21","duration":7,"periods":[{"title":"First","start":"2026-10-21","duration":7}]});
    let result = execute(&fixture, create);
    assert_eq!(result["project"]["result"]["authority"], "project");
    assert_eq!(
        result["project"]["result"]["configuration"]["iterations"][0]["startDate"],
        "2026-10-21"
    );
    assert!(
        fixture.state()["items"][0]["fieldValues"]["nodes"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn freshness() {
    let fixture = setup();
    let plan = decoded(&fixture.run(
        "prepare",
        &change(json!({"kind":"priority","field":"F_local","item":"ITEM_one","level":"High"})),
    ));
    fixture.change(|state| state["fields"][0]["options"][1]["name"] = "Human edit".into());
    refused(&fixture.run("apply", &plan), "concord.project.stale");
    assert!(fixture.state()["writes"].as_array().unwrap().is_empty());
}
