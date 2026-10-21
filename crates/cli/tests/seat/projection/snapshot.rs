use super::management::{Fixture, decoded, intent, refused};
use serde_json::{Value, json};

fn selection() -> Value {
    intent(json!({"kind":"candidates","issues":["I_one"],"present":true}))
}

fn writes(fixture: &Fixture) -> usize {
    fixture.state()["writes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|name| **name == "addProjectV2ItemById")
        .count()
}

#[test]
fn transient() {
    let fixture = Fixture::new("snapshot");
    let plan = decoded(&fixture.run("prepare", &selection()));
    let result = decoded(&fixture.run("apply", &plan));
    assert_eq!(result["project"]["outcome"], "confirmed-native-state");
    assert_eq!(fixture.state()["conflicts"], 1);
    assert_eq!(writes(&fixture), 1);
    let before = fixture.state()["writes"].clone();
    let current = decoded(&fixture.run("recover", &plan));
    assert_eq!(
        current["project"]["current"]["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        current["project"]["current"]["items"][0]["content"]["id"],
        "I_one"
    );
    assert_eq!(fixture.state()["writes"], before);
}

#[test]
fn exhausted() {
    let fixture = Fixture::new("snapshot-persistent");
    let plan = decoded(&fixture.run("prepare", &selection()));
    let error = refused(
        &fixture.run("apply", &plan),
        "concord.project.indeterminate",
    );
    assert_eq!(
        error["error"]["details"]["cause"]["code"],
        "concord.project.changed"
    );
    assert_eq!(fixture.state()["conflicts"], 3);
    assert_eq!(writes(&fixture), 1);
    assert_eq!(
        fixture.state()["statusUpdates"].as_array().unwrap().len(),
        1
    );
}

#[test]
fn preparation() {
    let fixture = Fixture::new("snapshot-prepare");
    fixture.change(|state| state["snapshot"] = 1.into());
    let plan = decoded(&fixture.run("prepare", &selection()));
    assert!(
        plan["plan"]["before"]["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(fixture.state()["conflicts"], 1);
    assert!(fixture.state()["writes"].as_array().unwrap().is_empty());
}

#[test]
fn context() {
    let fixture = Fixture::new("snapshot-context");
    let plan = decoded(&fixture.run(
        "prepare",
        &intent(json!({"kind":"candidates","issues":["I_one","I_two"],"present":true})),
    ));
    let error = refused(
        &fixture.run("apply", &plan),
        "concord.project.context.changed",
    );
    assert_eq!(
        error["error"]["details"]["completed"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(fixture.state()["conflicts"], 1);
    assert_eq!(writes(&fixture), 1);
    assert_eq!(fixture.state()["statusUpdates"][1]["id"], "U_foreign");
}
