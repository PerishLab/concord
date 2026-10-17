use super::super::world;
use serde_json::{Value, json};
use std::process::Output;

const CODE: &str = "concord.acceptance.target";

fn labelled(names: &[&str]) -> Value {
    let mut page = world::page();
    page["data"]["repository"]["issue"]["labels"] = world::connection(
        names.iter().map(|name| json!({"name": name})).collect(),
        None,
        names.len(),
    );
    page
}

fn run(page: &Value, operation: &str, input: Option<&Value>) -> Output {
    let scratch = tempfile::tempdir().unwrap();
    let provider = world::provider(scratch.path(), &[page.clone(), page.clone()]);
    world::invoke(scratch.path(), &provider, operation, input)
}

fn refusal(output: &Output) -> Value {
    assert!(!output.status.success());
    let failure: Value = serde_json::from_slice(&output.stderr)
        .unwrap_or_else(|_| panic!("{}", String::from_utf8_lossy(&output.stderr)));
    assert_eq!(failure["error"]["code"], CODE, "{failure}");
    assert_eq!(failure["error"]["cookbook"], CODE);
    failure["error"].clone()
}

#[test]
fn adoption() {
    let page = labelled(&["priority:urgent", "acceptance:source"]);
    let error = refusal(&run(&page, "show", None));
    assert_eq!(
        error["details"],
        json!({"label": "acceptance:source", "declared": null, "declaration": null})
    );
    assert!(
        error["message"]
            .as_str()
            .unwrap()
            .contains("declare target source to adopt the label")
    );
    refusal(&run(&page, "evaluate", None));
    let plan = world::decoded(&run(&page, "declare", Some(&world::intent())));
    assert_eq!(plan["plan"]["intent"]["prior"], Value::Null);
    let adopted = world::retained(&page, &plan, &["priority:urgent", "acceptance:source"]);
    let replies = [
        page.clone(),
        page,
        world::published(&plan),
        adopted.clone(),
        adopted.clone(),
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
        std::fs::read_to_string(scratch.path().join("calls")).unwrap(),
        "5\n"
    );
    let shown = world::decoded(&run(&adopted, "show", None));
    assert_eq!(shown["acceptance"]["target"], "source");
}

#[test]
fn mismatch() {
    let page = labelled(&["acceptance:release"]);
    let error = refusal(&run(&page, "declare", Some(&world::intent())));
    assert_eq!(error["details"]["label"], "acceptance:release");
    assert!(
        error["message"]
            .as_str()
            .unwrap()
            .contains("or remove acceptance:release before declaring another target")
    );
}

#[test]
fn disagreement() {
    let page = labelled(&[]);
    let plan = world::prepared(&page, "declare", &world::intent());
    let declared = world::retained(&page, &plan, &["acceptance:release"]);
    for operation in ["show", "evaluate"] {
        let error = refusal(&run(&declared, operation, None));
        assert_eq!(
            error["details"],
            json!({"label": "acceptance:release", "declared": "acceptance:source",
                "declaration": {"node": "intended",
                    "digest": concord_core::acceptance::digest(
                        plan["plan"]["intent"]["body"].as_str().unwrap())}})
        );
    }
    refusal(&run(&declared, "declare", Some(&world::intent())));
    let bare = world::retained(&page, &plan, &[]);
    let error = refusal(&run(&bare, "show", None));
    assert_eq!(error["details"]["label"], Value::Null);
    assert!(
        error["message"]
            .as_str()
            .unwrap()
            .contains("re-run acceptance apply with the plan that published intended")
    );
}
