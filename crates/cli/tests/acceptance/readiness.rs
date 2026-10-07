use super::{evaluate, world};
use serde_json::{Value, json};
use std::process::Output;

fn projection(page: &Value) -> Value {
    let mut issue = page["data"]["repository"]["issue"].clone();
    issue["title"] = "Bounded source acceptance".into();
    issue["updatedAt"] = issue["updated"].clone();
    issue["issueType"] = issue["kind"].clone();
    issue["repository"] = json!({"nameWithOwner": "PerishLab/concord"});
    issue["parent"] = Value::Null;
    issue["subIssuesSummary"] = json!({"total": 0, "completed": 0, "percentCompleted": 0});
    let empty = json!({"totalCount": 0, "nodes": [], "pageInfo": {"hasNextPage": false, "endCursor": null}});
    for name in [
        "subIssues",
        "blockedBy",
        "blocking",
        "timelineItems",
        "comments",
    ] {
        issue[name] = empty.clone();
    }
    json!({"data": {"repository": {"issue": issue}}})
}

fn invoke(before: &Value, held: &evaluate::World) -> Output {
    let scratch = tempfile::tempdir().unwrap();
    let bootstrap = crate::spawn::concord(scratch.path())
        .args([
            "--root",
            scratch.path().to_str().unwrap(),
            "issue",
            "bootstrap",
        ])
        .output()
        .unwrap();
    assert!(bootstrap.status.success());
    let mut replies = vec![
        before.clone(),
        json!({"data": {"r0": {"object": null}}}),
        held.page.clone(),
        held.page.clone(),
        held.relations.clone(),
        held.relations.clone(),
    ];
    if held.page["data"]["repository"]["issue"]["comments"]["total"] == 2 {
        replies.extend([
            held.fresh.clone(),
            held.fresh.clone(),
            held.page.clone(),
            held.page.clone(),
        ]);
    }
    replies.push(held.relations.clone());
    let provider = world::provider(scratch.path(), &replies);
    crate::spawn::concord(scratch.path())
        .args([
            "--root",
            scratch.path().to_str().unwrap(),
            "--json",
            "issue",
            "ready",
            "PerishLab/concord#83",
            "--github-command",
            provider.to_str().unwrap(),
        ])
        .output()
        .unwrap()
}

fn read(held: &evaluate::World) -> Value {
    world::decoded(&invoke(&projection(&held.page), held))["readiness"].clone()
}

#[test]
fn source() {
    let held = evaluate::World::new();
    let ready = read(&held);
    assert_eq!(ready["ready"], true);
    assert_eq!(ready["acceptance"], held.read());
    assert_eq!(ready["acceptance"]["verification"], "manual");
}

#[test]
fn absent() {
    let mut held = evaluate::World::new();
    let declaration = held.page["data"]["repository"]["issue"]["comments"]["nodes"][0].clone();
    held.page["data"]["repository"]["issue"]["comments"] =
        world::connection(vec![declaration], None, 1);
    let ready = read(&held);
    assert_eq!(ready["ready"], false);
    assert_eq!(ready["checks"]["acceptance_settled"], true);
    assert_eq!(ready["acceptance"]["verdict"], "unknown");
    assert!(
        ready["reasons"]
            .as_array()
            .unwrap()
            .iter()
            .any(|reason| reason.as_str().unwrap().contains("closure"))
    );
}

#[test]
fn invalidated() {
    let mut held = evaluate::World::new();
    held.fresh["data"]["repository"]["issue"]["edited"] = "2026-10-01T00:00:00Z".into();
    let ready = read(&held);
    assert_eq!(ready["ready"], false);
    assert_eq!(ready["acceptance"]["verdict"], "unmet");
}

#[test]
fn conditions() {
    let mut held = evaluate::World::new();
    held.relations["data"]["repository"]["issue"]["blockers"] =
        world::connection(vec![json!({"id": "I_blocker", "state": "OPEN"})], None, 1);
    let ready = read(&held);
    assert_eq!(ready["ready"], false);
    assert_eq!(ready["checks"]["blockers_closed"], false);
    assert_eq!(ready["acceptance"]["verdict"], "unmet");
}

#[test]
fn legacy() {
    let mut held = evaluate::World::new();
    let issue = &mut held.page["data"]["repository"]["issue"];
    issue["comments"] = world::connection(vec![], None, 0);
    issue["labels"] = world::connection(vec![], None, 0);
    let ready = read(&held);
    assert_eq!(ready["ready"], true);
    assert_eq!(ready["acceptance"]["target"], Value::Null);
    assert_eq!(ready["acceptance"]["verdict"], "unknown");
}

#[test]
fn moved() {
    let held = evaluate::World::new();
    let mut before = projection(&held.page);
    before["data"]["repository"]["issue"]["body"] = "Earlier provider body".into();
    world::refused(&invoke(&before, &held), "concord.acceptance.changed");
}

#[test]
fn identity() {
    let held = evaluate::World::new();
    let mut before = projection(&held.page);
    before["data"]["repository"]["issue"]["id"] = "I_foreign".into();
    world::refused(&invoke(&before, &held), "concord.acceptance.changed");
}

#[test]
fn kind() {
    let held = evaluate::World::new();
    let mut before = projection(&held.page);
    before["data"]["repository"]["issue"]["issueType"]["name"] = "Task".into();
    world::refused(&invoke(&before, &held), "concord.acceptance.changed");
}
