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

#[test]
fn diagnostics() {
    for (mode, changed) in [
        ("project", json!(["project"])),
        ("items", json!(["items"])),
        ("issues", json!(["issues"])),
        ("fields", json!(["fields", "columns"])),
        ("history-own", json!(["statusUpdates"])),
        ("history-peer", json!(["statusUpdates"])),
    ] {
        let fixture = Fixture::new(&format!("prepare-{mode}"));
        let action = if mode == "issues" {
            selection()
        } else {
            intent(json!({"kind":"record","body":"A bounded fixture note"}))
        };
        let error = refused(&fixture.run("prepare", &action), "concord.project.changed");
        assert_eq!(error["error"]["details"]["changed"], changed, "{mode}");
        assert_eq!(error["error"]["details"]["truncated"], false);
        assert!(!error.to_string().contains("DIAGNOSTIC_SECRET"));
        assert!(!error.to_string().contains("I_one"));
        let state = fixture.state();
        assert_eq!(state["starts"], 2);
        assert_eq!(state["calls"], if mode == "issues" { 18 } else { 16 });
        assert!(state["writes"].as_array().unwrap().is_empty());
    }
}

#[test]
fn acquisition() {
    let action = intent(json!({"kind":"record","body":"A bounded fixture note"}));
    let fixture = Fixture::new("prepare-key-order");
    decoded(&fixture.run("prepare", &action));
    assert_eq!(fixture.state()["starts"], 2);
    assert_eq!(fixture.state()["calls"], 16);
    for (mode, code, calls) in [
        ("prepare-failure", "concord.project.provider", 9),
        ("prepare-incomplete", "concord.project.schema", 10),
    ] {
        let fixture = Fixture::new(mode);
        let error = refused(&fixture.run("prepare", &action), code);
        assert!(error["error"]["details"].is_null());
        assert_eq!(fixture.state()["calls"], calls);
        assert!(fixture.state()["writes"].as_array().unwrap().is_empty());
    }
}
