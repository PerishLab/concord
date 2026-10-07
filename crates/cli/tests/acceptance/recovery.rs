use super::{apply, world};
use serde_json::json;

#[test]
fn amendment() {
    let (page, plan) = apply::amendment();
    let retained = world::retained(&page, &plan, &["acceptance:source", "priority:urgent"]);
    let removed = world::retained(&page, &plan, &["priority:urgent"]);
    let complete = world::retained(&page, &plan, &["priority:urgent", "acceptance:release"]);
    let replies = [
        retained.clone(),
        retained.clone(),
        world::catalog("release"),
        retained.clone(),
        retained.clone(),
        world::catalog("source"),
        retained.clone(),
        retained,
        world::altered(),
        removed.clone(),
        removed,
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
    assert_eq!(value["acceptance"]["recovery"]["target"], "release");
    assert_eq!(value["acceptance"]["recovery"]["step"], "complete");
    let removal = world::request(&scratch.path().join("request-9"));
    assert!(
        removal["query"]
            .as_str()
            .unwrap()
            .contains("removeLabelsFromLabelable")
    );
    assert_eq!(
        removal["variables"],
        json!({"issue": "I_issue", "labels": ["L_source"]})
    );
    let addition = world::request(&scratch.path().join("request-12"));
    assert!(
        addition["query"]
            .as_str()
            .unwrap()
            .contains("addLabelsToLabelable")
    );
    assert_eq!(
        addition["variables"],
        json!({"issue": "I_issue", "labels": ["L_release"]})
    );
    assert_eq!(
        std::fs::read_to_string(scratch.path().join("calls")).unwrap(),
        "14\n"
    );
}

#[test]
fn concurrent() {
    let (page, plan) = apply::amendment();
    let retained = world::retained(&page, &plan, &["acceptance:source"]);
    let complete = world::retained(&page, &plan, &["acceptance:release"]);
    let replies = [
        retained.clone(),
        retained.clone(),
        world::catalog("release"),
        retained.clone(),
        retained,
        world::catalog("source"),
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
        "8\n"
    );
}

#[test]
fn removed() {
    let (page, plan) = apply::amendment();
    let removed = world::retained(&page, &plan, &["priority:urgent"]);
    let complete = world::retained(&page, &plan, &["priority:urgent", "acceptance:release"]);
    let replies = [
        removed.clone(),
        removed.clone(),
        world::catalog("release"),
        removed.clone(),
        removed,
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
        "8\n"
    );
}

#[test]
fn indeterminate() {
    let page = world::page();
    let plan = world::prepared(&page, "declare", &world::intent());
    for present in [true, false] {
        let observed = if present {
            world::retained(&page, &plan, &["acceptance:source"])
        } else {
            page.clone()
        };
        let replies = [
            page.clone(),
            page.clone(),
            json!({"errors": [{"message": "unknown completion"}]}),
            observed.clone(),
            observed,
        ];
        let scratch = tempfile::tempdir().unwrap();
        let provider = world::provider(scratch.path(), &replies);
        let output = world::invoke(scratch.path(), &provider, "apply", Some(&plan));
        if present {
            assert_eq!(
                world::decoded(&output)["acceptance"]["recovery"]["step"],
                "complete"
            );
        } else {
            world::refused(&output, "concord.acceptance.partial");
            assert!(String::from_utf8_lossy(&output.stderr).contains("retry the same plan"));
        }
        assert_eq!(
            std::fs::read_to_string(scratch.path().join("calls")).unwrap(),
            "5\n"
        );
    }
}

#[test]
fn partial() {
    let (page, plan) = apply::amendment();
    let retained = world::retained(&page, &plan, &["acceptance:source"]);
    let removed = world::retained(&page, &plan, &[]);
    let replies = [
        retained.clone(),
        retained.clone(),
        world::catalog("release"),
        retained.clone(),
        retained.clone(),
        world::catalog("source"),
        retained.clone(),
        retained,
        world::altered(),
        removed.clone(),
        removed.clone(),
        json!({"errors": [{"message": "add rejected"}]}),
        removed.clone(),
        removed,
    ];
    let scratch = tempfile::tempdir().unwrap();
    let provider = world::provider(scratch.path(), &replies);
    let output = world::invoke(scratch.path(), &provider, "apply", Some(&plan));
    world::refused(&output, "concord.acceptance.partial");
    let message = String::from_utf8_lossy(&output.stderr);
    assert!(message.contains("concord.acceptance-plan/v1"));
    assert!(message.contains("last confirmed observation only"));
}

#[test]
fn changed() {
    let page = world::page();
    let plan = world::prepared(&page, "declare", &world::intent());
    let unknown = json!({"name": "acceptance:unknown"});
    let source = json!({"name": "acceptance:source"});
    let release = json!({"name": "acceptance:release"});
    for case in ["body", "closed", "unknown", "conflict"] {
        let mut observed = page.clone();
        let issue = &mut observed["data"]["repository"]["issue"];
        match case {
            "body" => issue["body"] = "Changed acceptance conditions.".into(),
            "closed" => issue["state"] = "CLOSED".into(),
            "unknown" => issue["labels"] = world::connection(vec![unknown.clone()], None, 1),
            "conflict" => {
                issue["labels"] = world::connection(vec![source.clone(), release.clone()], None, 2)
            }
            _ => unreachable!(),
        }
        let scratch = tempfile::tempdir().unwrap();
        let provider = world::provider(scratch.path(), &[observed.clone(), observed]);
        let output = world::invoke(scratch.path(), &provider, "apply", Some(&plan));
        assert!(!output.status.success());
        assert_eq!(
            std::fs::read_to_string(scratch.path().join("calls")).unwrap(),
            "2\n"
        );
    }
}
