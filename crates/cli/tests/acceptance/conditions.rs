use super::{evaluate::World, world};
use serde_json::json;

#[test]
fn obligations() {
    for field in ["children", "blockers"] {
        let mut world = World::new();
        world.relations["data"]["repository"]["issue"][field] =
            world::connection(vec![json!({"id": "I_related", "state": "OPEN"})], None, 1);
        let evaluation = world.read();
        assert_eq!(evaluation["verdict"], "unmet");
        let check = evaluation["checks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|check| check["name"] == field)
            .unwrap();
        assert_eq!(check["fact"]["verdict"], "unmet");
    }
}

#[test]
fn malformed() {
    for mode in ["duplicate", "state", "missing", "total", "errors", "issue"] {
        let world = World::new();
        let mut relations = world.relations.clone();
        let node = json!({"id": "I_related", "state": "CLOSED"});
        let errors = json!([{"message": "partial"}]);
        let issue = &mut relations["data"]["repository"]["issue"];
        let code = match mode {
            "duplicate" => {
                issue["children"] = world::connection(vec![node.clone(), node], None, 2);
                "identity"
            }
            "state" => {
                let mut node = node;
                node["state"] = "UNKNOWN".into();
                issue["blockers"] = world::connection(vec![node], None, 1);
                "reply"
            }
            "missing" => {
                issue.as_object_mut().unwrap().remove("children");
                "reply"
            }
            "total" => {
                issue["children"] = world::connection(vec![], None, 1);
                "reply"
            }
            "errors" => {
                relations["errors"] = errors;
                "provider"
            }
            _ => {
                issue["id"] = "foreign".into();
                "changed"
            }
        };
        let scratch = tempfile::tempdir().unwrap();
        let provider =
            world::provider(scratch.path(), &[world.page.clone(), world.page, relations]);
        world::refused(
            &world::invoke(scratch.path(), &provider, "evaluate", None),
            &format!("concord.acceptance.{code}"),
        );
    }
}

#[test]
fn pagination() {
    let world = World::new();
    let mut first = world.relations.clone();
    let mut second = first.clone();
    first["data"]["repository"]["issue"]["children"] = world::connection(
        vec![json!({"id": "I_one", "state": "CLOSED"})],
        Some("next"),
        2,
    );
    second["data"]["repository"]["issue"]["children"] =
        world::connection(vec![json!({"id": "I_two", "state": "OPEN"})], None, 2);
    let scratch = tempfile::tempdir().unwrap();
    let replies = [world.page.clone(), world.page, first, second];
    let provider = world::provider(scratch.path(), &replies);
    let mut command = world::command(scratch.path(), &provider, "evaluate");
    command.args(["--max-pages", "1"]);
    world::refused(&command.output().unwrap(), "concord.acceptance.truncated");
}

#[test]
fn continuation() {
    let world = World::new();
    let mut first = world.relations.clone();
    let mut second = first.clone();
    first["data"]["repository"]["issue"]["children"] = world::connection(
        vec![json!({"id": "I_one", "state": "CLOSED"})],
        Some("next"),
        2,
    );
    second["data"]["repository"]["issue"]["children"] =
        world::connection(vec![json!({"id": "I_two", "state": "OPEN"})], None, 2);
    let scratch = tempfile::tempdir().unwrap();
    let replies = [
        world.page.clone(),
        world.page.clone(),
        first.clone(),
        second.clone(),
        first.clone(),
        second.clone(),
        world.fresh.clone(),
        world.fresh,
        world.page.clone(),
        world.page,
        first,
        second,
    ];
    let provider = world::provider(scratch.path(), &replies);
    let value = world::decoded(&world::invoke(scratch.path(), &provider, "evaluate", None));
    assert_eq!(value["acceptance"]["evaluation"]["verdict"], "unmet");
    assert_eq!(
        world::request(&scratch.path().join("request-4"))["variables"]["children"],
        "next"
    );
    assert!(world::request(&scratch.path().join("request-4"))["variables"]["blockers"].is_null());
}

#[test]
fn moved() {
    let world = World::new();
    let mut later = world.relations.clone();
    later["data"]["repository"]["issue"]["blockers"] =
        world::connection(vec![json!({"id": "I_reopened", "state": "OPEN"})], None, 1);
    let scratch = tempfile::tempdir().unwrap();
    let replies = [
        world.page.clone(),
        world.page.clone(),
        world.relations.clone(),
        world.relations,
        world.fresh.clone(),
        world.fresh,
        world.page.clone(),
        world.page,
        later,
    ];
    let provider = world::provider(scratch.path(), &replies);
    world::refused(
        &world::invoke(scratch.path(), &provider, "evaluate", None),
        "concord.acceptance.changed",
    );
}
