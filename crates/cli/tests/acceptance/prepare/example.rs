use super::super::world;
use concord_core::acceptance::{Marker, Target, digest};
use serde_json::{Value, json};

fn failure(operation: &str) -> Value {
    let scratch = tempfile::tempdir().unwrap();
    let provider = world::provider(scratch.path(), &[]);
    let output = world::invoke(scratch.path(), &provider, operation, None);
    assert!(!output.status.success());
    assert!(!scratch.path().join("calls").exists());
    let value: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(value["error"]["code"], "concord.input.json");
    value["error"]["details"]["envelope"].clone()
}

fn marker(operation: &str) -> Value {
    failure(operation)["marker"].clone()
}

fn page(target: Target, promise: &str) -> (Value, String) {
    let declaration = Marker::Declaration {
        target,
        promise: promise.into(),
    };
    let body = format!("Current promise\n\n{}", declaration.render().unwrap());
    let mut page = world::page();
    let label = match target {
        Target::Source => "acceptance:source",
        Target::Release => "acceptance:release",
    };
    page["data"]["repository"]["issue"]["labels"] =
        world::connection(vec![json!({"name": label})], None, 1);
    page["data"]["repository"]["issue"]["comments"] =
        world::connection(vec![world::entry("root", &body, 1)], None, 1);
    (page, body)
}

#[test]
fn examples() {
    let declaration = marker("declare");
    let amendment = marker("amend");
    let closure = marker("judge");
    let root: Marker = serde_json::from_value(declaration).unwrap();
    assert!(matches!(
        root,
        Marker::Declaration {
            target: Target::Source,
            ..
        }
    ));
    let change: Marker = serde_json::from_value(amendment.clone()).unwrap();
    assert!(matches!(change, Marker::Amendment { .. }));
    let judgment: Marker = serde_json::from_value(closure.clone()).unwrap();
    assert!(matches!(judgment, Marker::Closure { .. }));
    assert_eq!(amendment["purpose"], "amendment");
    assert_eq!(closure["purpose"], "closure");
    for field in [
        "purpose",
        "target",
        "declaration",
        "judgment",
        "verification",
        "review",
        "evidence",
        "remaining",
        "release",
    ] {
        assert!(closure.get(field).is_some(), "missing {field}");
    }
}

#[test]
fn decode() {
    let amendment = marker("amend");
    assert!(
        amendment["promise"]
            .as_str()
            .unwrap()
            .starts_with("REPLACE_WITH_")
    );
    assert!(
        amendment["reason"]
            .as_str()
            .unwrap()
            .starts_with("REPLACE_WITH_")
    );
    assert!(
        amendment["predecessor"]["node"]
            .as_str()
            .unwrap()
            .starts_with("REPLACE_WITH_")
    );
    assert!(
        amendment["predecessor"]["digest"]
            .as_str()
            .unwrap()
            .contains("SHA256")
    );
    let closure = marker("judge");
    assert!(
        closure["declaration"]["digest"]
            .as_str()
            .unwrap()
            .contains("SHA256")
    );
    assert!(closure["review"].as_str().unwrap().contains("SHA256"));
    assert!(
        closure["evidence"][0]
            .as_str()
            .unwrap()
            .starts_with("REPLACE_WITH_")
    );
    assert_eq!(closure["judgment"], "unmet");
    assert_eq!(closure["verification"], "manual");
    assert_eq!(closure["remaining"].as_array().unwrap().len(), 1);
    assert!(closure["release"].is_null());
}

#[test]
fn prepared() {
    let (page, body) = page(Target::Source, "Current source promise.");
    let reference = json!({"node": "root", "digest": digest(&body)});

    let mut amendment = marker("amend");
    amendment["predecessor"] = reference.clone();
    amendment["promise"] = json!("Amended source promise.");
    amendment["reason"] = json!("The promise is now explicit.");
    let intent = json!({"prose": "Explicit amendment.", "marker": amendment});
    let scratch = tempfile::tempdir().unwrap();
    let provider = world::provider(scratch.path(), &[page.clone(), page.clone()]);
    let value = world::decoded(&world::invoke(
        scratch.path(),
        &provider,
        "amend",
        Some(&intent),
    ));
    assert_eq!(value["plan"]["intent"]["marker"]["purpose"], "amendment");

    let mut closure = marker("judge");
    closure["declaration"] = reference;
    closure["review"] = json!(digest("Current exact acceptance conditions."));
    closure["evidence"] = json!(["Reviewed source evidence recorded."]);
    closure["remaining"] = json!(["Release acceptance remains pending."]);
    let intent = json!({"prose": "Explicit manual judgment.", "marker": closure});
    let scratch = tempfile::tempdir().unwrap();
    let provider = world::provider(scratch.path(), &[page.clone(), page]);
    let value = world::decoded(&world::invoke(
        scratch.path(),
        &provider,
        "judge",
        Some(&intent),
    ));
    assert_eq!(value["plan"]["intent"]["marker"]["purpose"], "closure");
}

#[test]
fn refuse() {
    let (page, _) = page(Target::Source, "Current source promise.");
    for operation in ["amend", "judge"] {
        let intent = json!({"prose": "Unreviewed template.", "marker": marker(operation)});
        let scratch = tempfile::tempdir().unwrap();
        let provider = world::provider(scratch.path(), &[page.clone(), page.clone()]);
        world::refused(
            &world::invoke(scratch.path(), &provider, operation, Some(&intent)),
            "concord.acceptance.protocol",
        );
    }
}

#[test]
fn purpose() {
    let intent = json!({"prose": "Declaration template.", "marker": marker("declare")});
    for operation in ["amend", "judge"] {
        let scratch = tempfile::tempdir().unwrap();
        let provider = world::provider(scratch.path(), &[]);
        world::refused(
            &world::invoke(scratch.path(), &provider, operation, Some(&intent)),
            "concord.acceptance.purpose",
        );
        assert!(!scratch.path().join("calls").exists());
    }
}

#[test]
fn release() {
    let (page, body) = page(Target::Release, "Current release promise.");
    let closure = json!({
        "purpose": "closure",
        "target": "release",
        "declaration": {"node": "root", "digest": digest(&body)},
        "judgment": "satisfied",
        "verification": "manual",
        "review": digest("Current exact acceptance conditions."),
        "evidence": ["Reviewed release evidence recorded."],
        "remaining": [],
        "release": {
            "marker": "v0.0.0-test",
            "distribution": "Distribution completed for the fixture.",
            "inclusion": ["Installed --help readback recorded."]
        }
    });
    let intent = json!({"prose": "Explicit release judgment.", "marker": closure});
    let scratch = tempfile::tempdir().unwrap();
    let provider = world::provider(scratch.path(), &[page.clone(), page]);
    let value = world::decoded(&world::invoke(
        scratch.path(),
        &provider,
        "judge",
        Some(&intent),
    ));
    assert_eq!(value["plan"]["intent"]["marker"]["purpose"], "closure");
    assert_eq!(
        value["plan"]["intent"]["marker"]["release"]["marker"],
        "v0.0.0-test"
    );
}
