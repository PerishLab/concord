use super::management::{Fixture, decoded, intent, refused};
use serde_json::{Value, json};

fn candidates() -> Value {
    intent(json!({"kind":"candidates","issues":["I_one"],"present":true}))
}

fn count(fixture: &Fixture, operation: &str) -> usize {
    fixture.state()["writes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|value| **value == operation)
        .count()
}

#[test]
fn delayed() {
    let fixture = Fixture::new("delayed");
    let plan = decoded(&fixture.run("prepare", &candidates()));
    let result = decoded(&fixture.run("apply", &plan));
    assert_eq!(result["project"]["outcome"], "confirmed-native-state");
    assert_eq!(result["project"]["result"][0]["issue"], "I_one");
    assert_eq!(count(&fixture, "addProjectV2ItemById"), 1);
    assert_eq!(count(&fixture, "createProjectV2StatusUpdate"), 2);
    assert_eq!(fixture.state()["items"][0]["content"]["state"], "OPEN");
}

#[test]
fn configuration() {
    let fixture = Fixture::new("delayed-project");
    let change = intent(json!({"kind":"configure","changes":{"title":"Reviewed domain planning"}}));
    let plan = decoded(&fixture.run("prepare", &change));
    let result = decoded(&fixture.run("apply", &plan));
    assert_eq!(
        result["project"]["result"]["title"],
        "Reviewed domain planning"
    );
    assert_eq!(count(&fixture, "updateProjectV2"), 1);
}

#[test]
fn exhausted() {
    let fixture = Fixture::new("invisible");
    let plan = decoded(&fixture.run("prepare", &candidates()));
    let error = refused(
        &fixture.run("apply", &plan),
        "concord.project.indeterminate",
    );
    assert_eq!(error["error"]["details"]["unconfirmed_step"], 0);
    assert_eq!(fixture.state()["observations"], 3);
    assert_eq!(count(&fixture, "addProjectV2ItemById"), 1);
    assert_eq!(count(&fixture, "createProjectV2StatusUpdate"), 1);
    fixture.change(|state| state["mode"] = "ordinary".into());
    let writes = fixture.state()["writes"].clone();
    let recovered = decoded(&fixture.run("recover", &plan));
    assert_eq!(recovered["project"]["desired_state"], "observed");
    assert_eq!(fixture.state()["writes"], writes);
}

#[test]
fn context() {
    let fixture = Fixture::new("pending-context");
    let plan = decoded(&fixture.run(
        "prepare",
        &intent(json!({"kind":"candidates","issues":["I_one","I_two"],"present":true})),
    ));
    let error = refused(
        &fixture.run("apply", &plan),
        "concord.project.indeterminate",
    );
    assert_eq!(
        error["error"]["details"]["cause"]["code"],
        "concord.project.context.changed"
    );
    assert_eq!(fixture.state()["observations"], 1);
    assert_eq!(count(&fixture, "addProjectV2ItemById"), 1);
    assert_eq!(fixture.state()["items"].as_array().unwrap().len(), 1);
    assert_eq!(fixture.state()["statusUpdates"][1]["id"], "U_foreign");
}

#[test]
fn failure() {
    let fixture = Fixture::new("reader-failure");
    let plan = decoded(&fixture.run("prepare", &candidates()));
    refused(
        &fixture.run("apply", &plan),
        "concord.project.indeterminate",
    );
    assert_eq!(fixture.state()["observations"], 1);
    assert_eq!(count(&fixture, "addProjectV2ItemById"), 1);
}

#[test]
fn value() {
    let fixture = Fixture::new("delayed-value");
    fixture.change(|state| {
        state["fields"][0]["options"] = json!(["Urgent","High","Medium","Low"])
            .as_array().unwrap().iter()
            .map(|name|json!({"id":name,"name":name,"color":"BLUE","description":""}))
            .collect();
        let values = super::planning::connection(vec![], false);
        state["items"] = json!([{"id":"ITEM_one","type":"ISSUE","isArchived":false,"updatedAt":"original","project":{"id":"PVT_plan"},"content":state["issues"][0],"fieldValues":values}]);
    });
    let change = intent(
        json!({"kind":"planning","change":{"kind":"priority","field":"F_local","item":"ITEM_one","level":"High"}}),
    );
    let plan = decoded(&fixture.run("prepare", &change));
    let result = decoded(&fixture.run("apply", &plan));
    assert_eq!(result["project"]["result"]["selection"], "High");
    assert_eq!(count(&fixture, "updateProjectV2ItemFieldValue"), 1);
}

fn attachment(mode: &str) -> super::management::Fixture {
    let fixture = super::management::Fixture::new(mode);
    fixture.change(|state| state["origin"] = serde_json::json!({"__typename":"IssueFieldSingleSelect","id":"IFSS_phase","fullDatabaseId":"91","name":"Phase","dataType":"SINGLE_SELECT","visibility":"ORG_ONLY","options":[{"id":"IFSSO_v","name":"Verify","color":"GRAY","description":"Review"}]}));
    fixture
}

#[test]
fn attach() {
    use super::management::{decoded, intent, refused};
    let fixture = attachment("ordinary");
    let action = intent(serde_json::json!({"kind":"attach","origin":"IFSS_phase"}));
    let before = fixture.state();
    let plan = decoded(&fixture.run("prepare", &action));
    let result = decoded(&fixture.run("apply", &plan));
    assert_eq!(result["project"]["result"]["authority"], "issue");
    assert_eq!(
        result["project"]["result"]["issueField"]["id"],
        "IFSS_phase"
    );
    let current = fixture.state();
    assert_eq!(current["fields"][0], before["fields"][0]);
    assert_eq!(current["fields"][1], before["fields"][1]);
    assert_eq!(current["issues"], before["issues"]);
    let writes = current["writes"].clone();
    let plan = decoded(&fixture.run("prepare", &action));
    assert_eq!(
        decoded(&fixture.run("apply", &plan))["project"]["outcome"],
        "unchanged"
    );
    assert_eq!(fixture.state()["writes"], writes);
    let rename = intent(
        serde_json::json!({"kind":"rename","field":"F_attached","name":"Other","options":null}),
    );
    refused(
        &fixture.run("prepare", &rename),
        "concord.project.ownership",
    );
}

#[test]
fn attachunknown() {
    use super::management::{decoded, intent, refused};
    let fixture = attachment("attachment-unknown");
    let action = intent(serde_json::json!({"kind":"attach","origin":"IFSS_phase"}));
    let plan = decoded(&fixture.run("prepare", &action));
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
}

#[test]
fn attachforeign() {
    use super::management::{intent, refused};
    let fixture = attachment("ordinary");
    let action = intent(serde_json::json!({"kind":"attach","origin":"IFSS_phase"}));
    fixture.change(|state| state["origin_owner"] = "O_foreign".into());
    refused(&fixture.run("prepare", &action), "concord.project.identity");
    assert!(fixture.state()["writes"].as_array().unwrap().is_empty());
}
