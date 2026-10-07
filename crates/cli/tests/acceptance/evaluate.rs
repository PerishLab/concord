use super::world;
use concord_core::acceptance::{Judgment, Marker, Target, Verification, digest};
use serde_json::{Value, json};

pub(super) struct World {
    pub page: Value,
    pub fresh: Value,
    pub relations: Value,
}

impl World {
    pub fn new() -> Self {
        let mut page = world::page();
        page["data"]["repository"]["issue"]["body"] = "## Problem\nBounded change\n## Outcome\nVerified source\n## Acceptance\n- [x] Bounded result\n## Non-goals\nNo distribution\n".into();
        let body = page["data"]["repository"]["issue"]["body"]
            .as_str()
            .unwrap();
        let declaration = Marker::Declaration {
            target: Target::Source,
            promise: "Bounded source acceptance".into(),
        }
        .render()
        .unwrap();
        let closure = Marker::Closure {
            target: Target::Source,
            declaration: concord_core::acceptance::Reference {
                node: "IC_decl".into(),
                digest: digest(&declaration),
            },
            judgment: Judgment::Satisfied,
            verification: Verification::Manual,
            review: digest(body),
            evidence: vec!["https://example.org/source".into()],
            remaining: Vec::new(),
            release: None,
        }
        .render()
        .unwrap();
        let issue = &mut page["data"]["repository"]["issue"];
        issue["labels"] = world::connection(vec![json!({"name": "acceptance:source"})], None, 1);
        issue["comments"] = world::connection(
            vec![
                world::entry("IC_decl", &declaration, 1),
                world::entry("IC_close", &closure, 2),
            ],
            None,
            2,
        );
        let mut observed = issue.clone();
        observed["edited"] = Value::Null;
        observed["events"] = world::connection(vec![], None, 0);
        let fresh = json!({"data": {
            "repository": {"id": "R_concord", "issue": observed},
            "declaration": {"id": "IC_decl", "body": declaration, "created": "2026-09-29T00:00:00Z", "edited": null},
            "closure": {"id": "IC_close", "body": closure, "created": "2026-09-30T00:00:00Z", "edited": null},
        }});
        let mut relations = page.clone();
        let issue = &mut relations["data"]["repository"]["issue"];
        issue["children"] = world::connection(vec![], None, 0);
        issue["blockers"] = world::connection(vec![], None, 0);
        Self {
            page,
            fresh,
            relations,
        }
    }

    pub fn read(&self) -> Value {
        let scratch = tempfile::tempdir().unwrap();
        let replies = [
            self.page.clone(),
            self.page.clone(),
            self.relations.clone(),
            self.relations.clone(),
            self.fresh.clone(),
            self.fresh.clone(),
            self.page.clone(),
            self.page.clone(),
            self.relations.clone(),
        ];
        let provider = world::provider(scratch.path(), &replies);
        let output = world::invoke(scratch.path(), &provider, "evaluate", None);
        let decoded = world::decoded(&output);
        assert!(!scratch.path().join(".concord").exists());
        let request = world::request(&scratch.path().join("request-5"));
        assert_eq!(request["variables"]["declaration"], "IC_decl");
        assert_eq!(request["variables"]["closure"], "IC_close");
        decoded["acceptance"]["evaluation"].clone()
    }

    pub fn refuse(&self, code: &str) {
        let scratch = tempfile::tempdir().unwrap();
        let provider = world::provider(
            scratch.path(),
            &[
                self.page.clone(),
                self.page.clone(),
                self.relations.clone(),
                self.relations.clone(),
                self.fresh.clone(),
            ],
        );
        world::refused(
            &world::invoke(scratch.path(), &provider, "evaluate", None),
            code,
        );
    }
}

#[test]
fn current() {
    let evaluation = World::new().read();
    assert_eq!(evaluation["verification"], "manual");
    assert_eq!(evaluation["verdict"], "satisfied");
    assert_eq!(evaluation["checks"][2]["fact"]["verdict"], "satisfied");
    assert_eq!(evaluation["checks"][3]["name"], "sections");
}

#[test]
fn invalidated() {
    for mode in ["body", "declaration", "closure", "reopen"] {
        let mut world = World::new();
        let data = &mut world.fresh["data"];
        let event = json!({"id": "RE_one", "created": "2026-10-01T00:00:00Z"});
        match mode {
            "body" => data["repository"]["issue"]["edited"] = "2026-10-01T00:00:00Z".into(),
            "reopen" => {
                data["repository"]["issue"]["events"] = world::connection(vec![event], None, 1)
            }
            _ => data[mode]["edited"] = "2026-10-01T00:00:00Z".into(),
        }
        assert_eq!(
            world.read()["checks"][2]["fact"]["verdict"],
            "unmet",
            "{mode}"
        );
    }
}

#[test]
fn ambiguous() {
    let mut world = World::new();
    world.fresh["data"]["repository"]["issue"]["edited"] = "2026-09-30T00:00:00Z".into();
    assert_eq!(world.read()["checks"][2]["fact"]["verdict"], "unknown");
}

#[test]
fn identities() {
    for mode in [
        "comment",
        "issue",
        "body",
        "event",
        "errors",
        "timestamp",
        "missing",
    ] {
        let mut world = World::new();
        let data = &mut world.fresh["data"];
        let event = json!({"id": "RE_one", "created": "2026-10-01T00:00:00Z"});
        let errors = json!([{"message": "incomplete"}]);
        let code = match mode {
            "comment" => {
                data["closure"]["id"] = "foreign".into();
                "identity"
            }
            "body" => {
                data["closure"]["body"] = "edited".into();
                "identity"
            }
            "event" => {
                data["repository"]["issue"]["events"] =
                    world::connection(vec![event.clone(), event], None, 2);
                "identity"
            }
            "errors" => {
                world.fresh["errors"] = errors;
                "provider"
            }
            "timestamp" => {
                data["closure"]["created"] = "tomorrow".into();
                "reply"
            }
            "missing" => {
                data["repository"]["issue"]
                    .as_object_mut()
                    .unwrap()
                    .remove("edited");
                "reply"
            }
            _ => {
                data["repository"]["issue"]["id"] = "foreign".into();
                "changed"
            }
        };
        world.refuse(&format!("concord.acceptance.{code}"));
    }
}

#[test]
fn pagination() {
    let world = World::new();
    let mut first = world.fresh.clone();
    let mut second = first.clone();
    let older = json!({"id": "RE_old", "created": "2026-09-28T00:00:00Z"});
    let newer = json!({"id": "RE_new", "created": "2026-10-01T00:00:00Z"});
    first["data"]["repository"]["issue"]["events"] =
        world::connection(vec![older], Some("cursor"), 2);
    second["data"]["repository"]["issue"]["events"] = world::connection(vec![newer], None, 2);
    let scratch = tempfile::tempdir().unwrap();
    let replies = [
        world.page.clone(),
        world.page.clone(),
        world.relations.clone(),
        world.relations.clone(),
        first.clone(),
        second.clone(),
        first,
        second,
        world.page.clone(),
        world.page,
        world.relations,
    ];
    let provider = world::provider(scratch.path(), &replies);
    let value = world::decoded(&world::invoke(scratch.path(), &provider, "evaluate", None));
    assert_eq!(value["acceptance"]["evaluation"]["verdict"], "unmet");
    let request = world::request(&scratch.path().join("request-6"));
    assert_eq!(request["variables"]["after"], "cursor");
}

#[test]
fn incomplete() {
    let world = World::new();
    let mut fresh = world.fresh.clone();
    fresh["data"]["repository"]["issue"]["events"] = world::connection(vec![], Some("cursor"), 1);
    let scratch = tempfile::tempdir().unwrap();
    let replies = [
        world.page.clone(),
        world.page,
        world.relations.clone(),
        world.relations,
        fresh,
    ];
    let provider = world::provider(scratch.path(), &replies);
    let mut command = world::command(scratch.path(), &provider, "evaluate");
    command.args(["--max-pages", "1"]);
    world::refused(&command.output().unwrap(), "concord.acceptance.truncated");
}

#[test]
fn changed() {
    let world = World::new();
    let mut second = world.fresh.clone();
    second["data"]["repository"]["issue"]["edited"] = "2026-10-01T00:00:00Z".into();
    let scratch = tempfile::tempdir().unwrap();
    let replies = [
        world.page.clone(),
        world.page,
        world.relations.clone(),
        world.relations,
        world.fresh,
        second,
    ];
    let provider = world::provider(scratch.path(), &replies);
    world::refused(
        &world::invoke(scratch.path(), &provider, "evaluate", None),
        "concord.acceptance.changed",
    );
}

#[test]
fn calendar() {
    for timestamp in [
        "2026-02-29T00:00:00Z",
        "2026-13-01T00:00:00Z",
        "2026-10-01T24:00:00Z",
        "2026-10-01T00:60:00Z",
        "2026-10-01T00:00:60Z",
    ] {
        let mut world = World::new();
        world.fresh["data"]["closure"]["created"] = timestamp.into();
        world.refuse("concord.acceptance.reply");
    }
}
