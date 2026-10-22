use super::super::world;
use concord_core::acceptance::{Marker, Target, digest};
use serde_json::{Value, json};

fn judgment() -> (Value, Value) {
    let mut page = world::page();
    let body = Marker::Declaration {
        target: Target::Source,
        promise: "Source acceptance.".into(),
    }
    .render()
    .unwrap();
    page["data"]["repository"]["issue"]["labels"] =
        world::connection(vec![json!({"name": "acceptance:source"})], None, 1);
    page["data"]["repository"]["issue"]["comments"] =
        world::connection(vec![world::entry("root", &body, 1)], None, 1);
    let review = digest(
        page["data"]["repository"]["issue"]["body"]
            .as_str()
            .unwrap(),
    );
    let intent = json!({"prose": "Explicit manual source review.", "marker": {
        "purpose": "closure", "target": "source",
        "declaration": {"node": "root", "digest": digest(&body)},
        "judgment": "unmet", "verification": "manual", "review": review,
        "evidence": [], "remaining": ["Merge is still outstanding."], "release": null
    }});
    (page, intent)
}

fn reads(scratch: &std::path::Path) {
    assert_eq!(
        std::fs::read_to_string(scratch.join("calls")).unwrap(),
        "2\n"
    );
    for index in 1..=2 {
        let request = world::request(&scratch.join(format!("request-{index}")));
        assert!(!request["query"].as_str().unwrap().contains("mutation"));
    }
}

#[test]
fn prepare() {
    let (page, intent) = judgment();
    let valid = world::prepared(&page, "judge", &intent);
    for review in [
        digest("Previous conditions."),
        valid["plan"]["review"].as_str().unwrap().into(),
        digest("Current exact acceptance conditions.\n"),
    ] {
        let scratch = tempfile::tempdir().unwrap();
        let provider = world::provider(scratch.path(), &[page.clone(), page.clone()]);
        let mut invalid = intent.clone();
        invalid["marker"]["review"] = review.into();
        world::refused(
            &world::invoke(scratch.path(), &provider, "judge", Some(&invalid)),
            "concord.acceptance.review",
        );
        reads(scratch.path());
    }
}

#[test]
fn apply() {
    let (page, intent) = judgment();
    let mut plan = world::prepared(&page, "judge", &intent);
    let prior = intent["marker"]["review"].as_str().unwrap();
    let review = digest("Previous conditions.");
    plan["plan"]["intent"]["marker"]["review"] = review.clone().into();
    plan["plan"]["intent"]["body"] = plan["plan"]["intent"]["body"]
        .as_str()
        .unwrap()
        .replace(prior, &review)
        .into();
    let scratch = tempfile::tempdir().unwrap();
    let provider = world::provider(scratch.path(), &[page.clone(), page]);
    world::refused(
        &world::invoke(scratch.path(), &provider, "apply", Some(&plan)),
        "concord.acceptance.review",
    );
    reads(scratch.path());
}

#[test]
fn current() {
    for satisfied in [false, true] {
        publish(satisfied);
    }
}

fn publish(satisfied: bool) {
    let (page, mut intent) = judgment();
    if satisfied {
        intent["marker"]["judgment"] = "satisfied".into();
        intent["marker"]["evidence"] = json!(["https://github.com/PerishLab/concord/pull/84"]);
        intent["marker"]["remaining"] = json!([]);
    }
    let plan = world::prepared(&page, "judge", &intent);
    let complete = world::retained(&page, &plan, &["acceptance:source"]);
    let replies = [
        page.clone(),
        page,
        world::published(&plan),
        complete.clone(),
        complete.clone(),
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
    assert_eq!(value["acceptance"]["snapshot"]["header"]["state"], "OPEN");
    let publication = world::request(&scratch.path().join("request-3"));
    assert_eq!(
        publication["variables"]["body"],
        plan["plan"]["intent"]["body"]
    );
    let provider = world::provider(scratch.path(), &[complete.clone(), complete]);
    std::fs::remove_file(scratch.path().join("calls")).unwrap();
    let value = world::decoded(&world::invoke(
        scratch.path(),
        &provider,
        "apply",
        Some(&plan),
    ));
    assert_eq!(value["acceptance"]["recovery"]["step"], "complete");
    reads(scratch.path());
}

#[test]
fn changed() {
    let (mut page, intent) = judgment();
    let plan = world::prepared(&page, "judge", &intent);
    page["data"]["repository"]["issue"]["body"] = "Changed conditions.".into();
    let scratch = tempfile::tempdir().unwrap();
    let provider = world::provider(scratch.path(), &[page.clone(), page]);
    world::refused(
        &world::invoke(scratch.path(), &provider, "apply", Some(&plan)),
        "concord.acceptance.changed",
    );
    reads(scratch.path());
}
