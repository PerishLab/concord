use super::world;
use serde_json::{Value, json};

#[test]
fn declaration() {
    let page = world::page();
    let plan = world::prepared(&page, "declare", &world::intent());
    let retained = world::retained(&page, &plan, &["priority:urgent"]);
    let complete = world::retained(&page, &plan, &["priority:urgent", "acceptance:source"]);
    let replies = [
        page.clone(),
        page,
        world::published(&plan),
        retained.clone(),
        retained.clone(),
        world::catalog("source"),
        retained.clone(),
        retained,
        world::altered(),
        complete.clone(),
        complete,
    ];
    let scratch = tempfile::tempdir().unwrap();
    let provider = world::provider(scratch.path(), &replies);
    let value = world::decoded(&world::invoke(
        scratch.path(),
        &provider,
        "apply",
        Some(&plan),
    ));
    assert_eq!(value["acceptance"]["recovery"]["step"], "complete");
    assert_eq!(
        value["acceptance"]["recovery"]["reference"]["node"],
        "intended"
    );
    assert_eq!(
        value["acceptance"]["snapshot"]["labels"],
        json!(["acceptance:source", "priority:urgent"])
    );
    assert!(!scratch.path().join(".concord").exists());
    let publication = world::request(&scratch.path().join("request-3"));
    assert!(
        publication["query"]
            .as_str()
            .unwrap()
            .contains("addComment")
    );
    assert_eq!(
        publication["variables"]["body"],
        plan["plan"]["intent"]["body"]
    );
    let mutation = world::request(&scratch.path().join("request-9"));
    assert!(
        mutation["query"]
            .as_str()
            .unwrap()
            .contains("labelable{__typename ... on Issue{id}}")
    );
    assert!(
        mutation["query"]
            .as_str()
            .unwrap()
            .contains("addLabelsToLabelable")
    );
    assert_eq!(
        mutation["variables"],
        json!({"issue": "I_issue", "labels": ["L_source"]})
    );
    assert_eq!(
        std::fs::read_to_string(scratch.path().join("calls")).unwrap(),
        "11\n"
    );
}

#[test]
fn creation() {
    let page = world::page();
    let plan = world::prepared(&page, "declare", &world::intent());
    let retained = world::retained(&page, &plan, &[]);
    let complete = world::retained(&page, &plan, &["acceptance:source"]);
    let created = json!({"label": {"id": "L_source", "name": "acceptance:source"}});
    let replies = [
        retained.clone(),
        retained.clone(),
        json!({"data": {"repository": {"id": "R_concord", "label": null}}}),
        json!({"data": {"created": created}}),
        retained.clone(),
        retained,
        world::altered(),
        complete.clone(),
        complete,
    ];
    let scratch = tempfile::tempdir().unwrap();
    let provider = world::provider(scratch.path(), &replies);
    let value = world::decoded(&world::invoke(
        scratch.path(),
        &provider,
        "apply",
        Some(&plan),
    ));
    assert_eq!(value["acceptance"]["recovery"]["step"], "complete");
    assert_eq!(
        std::fs::read_to_string(scratch.path().join("calls")).unwrap(),
        "9\n"
    );
}

#[test]
fn repeated() {
    let page = world::page();
    let plan = world::prepared(&page, "declare", &world::intent());
    let complete = world::retained(&page, &plan, &["acceptance:source"]);
    let scratch = tempfile::tempdir().unwrap();
    let provider = world::provider(scratch.path(), &[complete.clone(), complete]);
    let value = world::decoded(&world::invoke(
        scratch.path(),
        &provider,
        "apply",
        Some(&plan),
    ));
    assert_eq!(value["acceptance"]["recovery"]["step"], "complete");
    assert_eq!(
        std::fs::read_to_string(scratch.path().join("calls")).unwrap(),
        "2\n"
    );
}

#[test]
fn consent() {
    let scratch = tempfile::tempdir().unwrap();
    let provider = world::provider(scratch.path(), &[]);
    world::refused(
        &world::command(scratch.path(), &provider, "apply")
            .output()
            .unwrap(),
        "concord.acceptance.consent",
    );
    assert!(!scratch.path().join("calls").exists());
}

#[test]
fn binding() {
    for case in ["version", "issue", "review", "footer", "field"] {
        let page = world::page();
        let mut plan = world::prepared(&page, "declare", &world::intent());
        match case {
            "version" => plan["version"] = 2.into(),
            "issue" => plan["plan"]["issue"]["coordinate"]["number"] = 84.into(),
            "review" => plan["plan"]["review"] = "changed".into(),
            "footer" => plan["plan"]["execution"]["session"] = "another".into(),
            "field" => plan["authority"] = true.into(),
            _ => unreachable!(),
        }
        let scratch = tempfile::tempdir().unwrap();
        let provider = world::provider(scratch.path(), &[]);
        let output = world::invoke(scratch.path(), &provider, "apply", Some(&plan));
        assert!(!output.status.success());
        assert!(!scratch.path().join("calls").exists());
    }
}

pub(super) fn amendment() -> (Value, Value) {
    let mut page = world::page();
    let body = concord_core::acceptance::Marker::Declaration {
        target: concord_core::acceptance::Target::Source,
        promise: "Source acceptance.".into(),
    }
    .render()
    .unwrap();
    page["data"]["repository"]["issue"]["comments"] =
        world::connection(vec![world::entry("root", &body, 1)], None, 1);
    page["data"]["repository"]["issue"]["labels"] =
        world::connection(vec![json!({"name": "acceptance:source"})], None, 1);
    let intent = json!({"prose": "Explicit release amendment.", "marker": {
        "purpose": "amendment", "target": "release", "promise": "Installed stable verification.",
        "predecessor": {"node": "root", "digest": concord_core::acceptance::digest(&body)},
        "reason": "Installed consumption is now required."
    }});
    let plan = world::prepared(&page, "amend", &intent);
    (page, plan)
}
