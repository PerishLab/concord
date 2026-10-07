use super::world;
use serde_json::json;

#[test]
fn independent() {
    let scratch = tempfile::tempdir().unwrap();
    let page = world::page();
    let provider = world::provider(scratch.path(), &[page.clone(), page]);
    let output = world::invoke(scratch.path(), &provider, "show", None);
    let value = world::decoded(&output);
    assert!(value["acceptance"]["target"].is_null());
    assert_eq!(value["acceptance"]["snapshot"]["header"]["node"], "I_issue");
    assert!(!scratch.path().join(".concord").exists());
    assert_eq!(
        std::fs::read_to_string(scratch.path().join("calls")).unwrap(),
        "2\n"
    );
}

#[test]
fn pagination() {
    let scratch = tempfile::tempdir().unwrap();
    let mut first = world::page();
    first["data"]["repository"]["issue"]["labels"] =
        world::connection(vec![json!({"name": "priority:normal"})], None, 1);
    first["data"]["repository"]["issue"]["comments"] = world::connection(
        vec![world::entry("one", "Ordinary progress.", 1)],
        Some("first"),
        2,
    );
    let mut second = first.clone();
    second["data"]["repository"]["issue"]["comments"] =
        world::connection(vec![world::entry("two", "More progress.", 2)], None, 2);
    let provider = world::provider(
        scratch.path(),
        &[first.clone(), second.clone(), first, second],
    );
    let output = world::invoke(scratch.path(), &provider, "show", None);
    let value = world::decoded(&output);
    assert_eq!(
        value["acceptance"]["snapshot"]["entries"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        value["acceptance"]["snapshot"]["labels"],
        json!(["priority:normal"])
    );
    let request: serde_json::Value =
        serde_json::from_slice(&std::fs::read(scratch.path().join("request")).unwrap()).unwrap();
    assert_eq!(request["variables"]["comments"], "first");
}

#[test]
fn bounds() {
    let scratch = tempfile::tempdir().unwrap();
    let provider = world::provider(scratch.path(), &[]);
    let mut command = world::command(scratch.path(), &provider, "show");
    command.args(["--max-pages", "0"]);
    world::refused(&command.output().unwrap(), "concord.acceptance.bounds");
    assert!(!scratch.path().join("calls").exists());
}

#[test]
fn changed() {
    let scratch = tempfile::tempdir().unwrap();
    let first = world::page();
    let mut second = first.clone();
    second["data"]["repository"]["issue"]["body"] = "Externally edited conditions.".into();
    let provider = world::provider(scratch.path(), &[first, second]);
    world::refused(
        &world::invoke(scratch.path(), &provider, "show", None),
        "concord.acceptance.changed",
    );
}

#[test]
fn incomplete() {
    for mode in ["truncated", "cursor", "total", "errors"] {
        let scratch = tempfile::tempdir().unwrap();
        let mut page = world::page();
        page["data"]["repository"]["issue"]["comments"] = match mode {
            "truncated" | "cursor" => world::connection(vec![], Some("next"), 1),
            "total" => world::connection(vec![], None, 1),
            _ => world::connection(vec![], None, 0),
        };
        if mode == "errors" {
            page["errors"] = json!([{"message": "Partial provider result"}]);
        }
        let provider = world::provider(scratch.path(), &[page.clone(), page]);
        let mut command = world::command(scratch.path(), &provider, "show");
        command.args(["--max-pages", if mode == "truncated" { "1" } else { "2" }]);
        let output = command.output().unwrap();
        let code = match mode {
            "truncated" => "truncated",
            "cursor" => "cursor",
            "total" => "reply",
            _ => "provider",
        };
        world::refused(&output, &format!("concord.acceptance.{code}"));
    }
}

#[test]
fn identities() {
    for mode in ["issue", "duplicate", "foreign"] {
        let scratch = tempfile::tempdir().unwrap();
        let mut page = world::page();
        let mut entry = world::entry("one", "Progress.", 1);
        let nodes = match mode {
            "duplicate" => vec![entry.clone(), entry],
            "foreign" => {
                entry["url"] =
                    "https://github.com/PerishLab/concord/issues/84#issuecomment-1".into();
                vec![entry]
            }
            _ => {
                page["data"]["repository"]["issue"]["number"] = 84.into();
                vec![]
            }
        };
        let count = nodes.len();
        page["data"]["repository"]["issue"]["comments"] = world::connection(nodes, None, count);
        let provider = world::provider(scratch.path(), &[page]);
        world::refused(
            &world::invoke(scratch.path(), &provider, "show", None),
            "concord.acceptance.identity",
        );
    }
}

#[test]
fn versions() {
    let scratch = tempfile::tempdir().unwrap();
    let mut page = world::page();
    page["data"]["repository"]["issue"]["comments"] = world::connection(
        vec![world::entry(
            "unknown",
            "<!-- concord.acceptance/v99\n{}\n-->",
            1,
        )],
        None,
        1,
    );
    let provider = world::provider(scratch.path(), &[page.clone(), page]);
    world::refused(
        &world::invoke(scratch.path(), &provider, "show", None),
        "concord.acceptance.protocol",
    );
}
