use super::super::world;
use serde_json::{Value, json};

fn manifest(text: Option<&str>) -> Value {
    let object = text.map(|text| json!({"oid": "a".repeat(40), "text": text}));
    json!({"data": {"repository": {"id": "R_concord", "object": object}}})
}

fn prepared(replies: [Value; 2], operation: &str) -> Value {
    let scratch = tempfile::tempdir().unwrap();
    let mut page = world::page();
    let mut intent = world::intent();
    intent["marker"]["target"] = "release".into();
    if operation == "amend" {
        let root = world::prepared(&page, "declare", &world::intent());
        let body = root["plan"]["intent"]["body"].as_str().unwrap();
        page["data"]["repository"]["issue"]["comments"] =
            world::connection(vec![world::entry("root", body, 1)], None, 1);
        page["data"]["repository"]["issue"]["labels"] =
            world::connection(vec![json!({"name": "acceptance:source"})], None, 1);
        intent["marker"] = json!({"purpose": "amendment", "target": "release", "predecessor": {"node": "root", "digest": concord_core::acceptance::digest(body)}, "promise": "Containing stable and installed behavior.", "reason": "Explicit consumer promise."});
    }
    let provider = world::provider(
        scratch.path(),
        &[page.clone(), page, replies[0].clone(), replies[1].clone()],
    );
    let value = world::decoded(&world::invoke(
        scratch.path(),
        &provider,
        operation,
        Some(&intent),
    ));
    assert_eq!(value["plan"]["schema"], "concord.acceptance-plan/v2");
    assert_eq!(value["plan"]["intent"]["marker"]["target"], "release");
    assert_eq!(value["plan"]["authority"]["repository"], "R_concord");
    assert!(!scratch.path().join(".concord").exists());
    assert_eq!(
        std::fs::read_to_string(scratch.path().join("calls")).unwrap(),
        "4\n"
    );
    value
}

#[test]
fn states() {
    for (text, state) in [
        (None, "missing"),
        (Some("[identity]\nns=\"demo\"\n"), "missing"),
        (
            Some("[release]\nauthority=\"https://releases.example.org\"\n"),
            "declared",
        ),
        (Some("[release]\nauthority=\"PRIVATE_SECRET\"\n"), "invalid"),
        (Some("[release]\nauthority='PRIVATE_SECRET\n"), "invalid"),
    ] {
        for operation in ["declare", "amend"] {
            let reply = manifest(text);
            let value = prepared([reply.clone(), reply], operation);
            assert_eq!(value["plan"]["authority"]["status"], state);
            assert!(!value.to_string().contains("PRIVATE_SECRET"));
        }
    }
}

#[test]
fn unknown() {
    for mode in [
        "provider",
        "identity",
        "missing",
        "object",
        "malformed",
        "moving",
    ] {
        let first = manifest(Some(
            "[release]\nauthority=\"https://releases.example.org\"\n",
        ));
        let mut second = first.clone();
        match mode {
            "provider" => second["errors"] = json!([{"message": "PRIVATE_SECRET"}]),
            "identity" => second["data"]["repository"]["id"] = "PRIVATE_SECRET".into(),
            "missing" => second["data"]["repository"] = Value::Null,
            "object" => second["data"]["repository"]["object"]["oid"] = "PRIVATE_SECRET".into(),
            "malformed" => second = json!("PRIVATE_SECRET"),
            "moving" => second["data"]["repository"]["object"]["oid"] = "b".repeat(40).into(),
            _ => unreachable!(),
        }
        let value = prepared([first, second], "declare");
        assert_eq!(value["plan"]["authority"]["status"], "unknown");
        assert_eq!(value["plan"]["authority"]["manifest"], Value::Null);
        assert!(!value.to_string().contains("PRIVATE_SECRET"));
    }
}

#[test]
fn apply() {
    let reply = manifest(None);
    let plan = prepared([reply.clone(), reply], "declare");
    let scratch = tempfile::tempdir().unwrap();
    let page = world::page();
    let retained = world::retained(&page, &plan, &["acceptance:release"]);
    let provider = world::provider(scratch.path(), &[retained.clone(), retained]);
    let value = world::decoded(&world::invoke(
        scratch.path(),
        &provider,
        "apply",
        Some(&plan),
    ));
    assert_eq!(value["acceptance"]["recovery"]["step"], "complete");
    assert_eq!(
        value["acceptance"]["plan"]["plan"]["schema"],
        "concord.acceptance-plan/v2"
    );
}

#[test]
fn legacy() {
    let reply = manifest(None);
    let mut plan = prepared([reply.clone(), reply], "declare");
    plan["plan"]["schema"] = "concord.acceptance-plan/v1".into();
    plan["plan"].as_object_mut().unwrap().remove("authority");
    let scratch = tempfile::tempdir().unwrap();
    let page = world::page();
    let retained = world::retained(&page, &plan, &["acceptance:release"]);
    let provider = world::provider(scratch.path(), &[retained.clone(), retained]);
    let value = world::decoded(&world::invoke(
        scratch.path(),
        &provider,
        "apply",
        Some(&plan),
    ));
    assert_eq!(value["acceptance"]["recovery"]["step"], "complete");
}

#[test]
fn shape() {
    let reply = manifest(None);
    let plan = prepared([reply.clone(), reply], "declare");
    for mode in ["legacy", "repository", "object", "message"] {
        let scratch = tempfile::tempdir().unwrap();
        let mut altered = plan.clone();
        match mode {
            "legacy" => altered["plan"]["schema"] = "concord.acceptance-plan/v1".into(),
            "repository" => altered["plan"]["authority"]["repository"] = "R_other".into(),
            "object" => altered["plan"]["authority"]["manifest"] = "PRIVATE_SECRET".into(),
            "message" => altered["plan"]["authority"]["message"] = "PRIVATE_SECRET".into(),
            _ => unreachable!(),
        }
        let provider = world::provider(scratch.path(), &[]);
        let output = world::invoke(scratch.path(), &provider, "apply", Some(&altered));
        world::refused(&output, "concord.acceptance.plan");
        assert!(!scratch.path().join("calls").exists());
        assert!(!String::from_utf8_lossy(&output.stderr).contains("PRIVATE_SECRET"));
    }
}
