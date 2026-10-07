use super::world;
use concord_core::acceptance::{Marker, Plan, Step, Target, digest};
use serde_json::json;

#[test]
fn declaration() {
    let scratch = tempfile::tempdir().unwrap();
    let mut page = world::page();
    page["data"]["repository"]["issue"]["labels"] =
        world::connection(vec![json!({"name": "priority:normal"})], None, 1);
    let provider = world::provider(scratch.path(), &[page.clone(), page]);
    let output = world::invoke(scratch.path(), &provider, "declare", Some(&world::intent()));
    let value = world::decoded(&output);
    assert_eq!(value["plan"]["schema"], "concord.acceptance-plan/v1");
    assert_eq!(value["plan"]["issue"]["node"], "I_issue");
    assert_eq!(value["plan"]["issue"]["repository"], "R_concord");
    assert_eq!(value["plan"]["execution"]["agent"], "codex");
    let plan: Plan = serde_json::from_value(value["plan"]["intent"].clone()).unwrap();
    assert!(
        plan.body()
            .unwrap()
            .contains("<!-- concord.issue-comment/v1")
    );
    assert_eq!(
        Marker::parse(plan.body().unwrap())
            .unwrap()
            .unwrap()
            .target(),
        Target::Source
    );
    assert_eq!(
        plan.recover(&["priority:normal".into()], &[]).unwrap().step,
        Step::Publish
    );
    assert!(!scratch.path().join(".concord").exists());
    assert_eq!(
        std::fs::read_to_string(scratch.path().join("calls")).unwrap(),
        "2\n"
    );
}

#[test]
fn transitions() {
    for operation in ["amend", "judge"] {
        let scratch = tempfile::tempdir().unwrap();
        let root = Marker::Declaration {
            target: Target::Source,
            promise: "Source acceptance.".into(),
        };
        let body = format!("Current promise\n\n{}", root.render().unwrap());
        let reference = json!({"node": "root", "digest": digest(&body)});
        let marker = if operation == "amend" {
            json!({"purpose": "amendment", "target": "release", "promise": "Installed stable verification.", "predecessor": reference, "reason": "Installed consumption is required."})
        } else {
            json!({"purpose": "closure", "target": "source", "declaration": reference, "judgment": "unmet", "verification": "manual", "review": digest("manual review basis"), "evidence": [], "remaining": ["Merge is still outstanding."], "release": null})
        };
        let mut page = world::page();
        page["data"]["repository"]["issue"]["labels"] =
            world::connection(vec![json!({"name": "acceptance:source"})], None, 1);
        page["data"]["repository"]["issue"]["comments"] =
            world::connection(vec![world::entry("root", &body, 1)], None, 1);
        let provider = world::provider(scratch.path(), &[page.clone(), page]);
        let intent = json!({"prose": "Explicit manual acceptance update.", "marker": marker});
        let value = world::decoded(&world::invoke(
            scratch.path(),
            &provider,
            operation,
            Some(&intent),
        ));
        assert!(
            value["plan"]["semantics"]
                .as_str()
                .unwrap()
                .contains("no provider write")
        );
        assert_eq!(
            value["plan"]["intent"]["marker"]["purpose"],
            if operation == "amend" {
                "amendment"
            } else {
                "closure"
            }
        );
    }
}

#[test]
fn operator() {
    for session in [None, Some("invalid session!")] {
        let scratch = tempfile::tempdir().unwrap();
        let provider = world::provider(scratch.path(), &[]);
        let input = scratch.path().join("input.json");
        std::fs::write(&input, serde_json::to_vec(&world::intent()).unwrap()).unwrap();
        let mut command = world::command(scratch.path(), &provider, "declare");
        command.args(["--input", input.to_str().unwrap()]);
        if let Some(session) = session {
            command.env("CODEX_THREAD_ID", session);
        }
        world::refused(&command.output().unwrap(), "concord.issue.comment.operator");
        assert!(!scratch.path().join("calls").exists());
    }
}

#[test]
fn validation() {
    for mode in ["purpose", "prose", "limit"] {
        let scratch = tempfile::tempdir().unwrap();
        let provider = world::provider(scratch.path(), &[]);
        let mut intent = world::intent();
        let (operation, code) = match mode {
            "purpose" => ("amend", "concord.acceptance.purpose"),
            "prose" => {
                intent["prose"] = "  ".into();
                ("declare", "concord.acceptance.prose")
            }
            "limit" => {
                intent["prose"] = "a".repeat(65_536).into();
                ("declare", "concord.issue.comment.limit")
            }
            _ => unreachable!(),
        };
        let output = world::invoke(scratch.path(), &provider, operation, Some(&intent));
        world::refused(&output, code);
        assert!(!scratch.path().join("calls").exists());
    }
}

#[test]
fn conflict() {
    let scratch = tempfile::tempdir().unwrap();
    let mut page = world::page();
    page["data"]["repository"]["issue"]["labels"] = world::connection(
        vec![
            json!({"name": "acceptance:source"}),
            json!({"name": "acceptance:release"}),
        ],
        None,
        2,
    );
    let provider = world::provider(scratch.path(), &[page.clone(), page]);
    world::refused(
        &world::invoke(scratch.path(), &provider, "declare", Some(&world::intent())),
        "concord.acceptance.protocol",
    );
}

#[test]
fn closed() {
    let scratch = tempfile::tempdir().unwrap();
    let mut page = world::page();
    page["data"]["repository"]["issue"]["state"] = "CLOSED".into();
    let provider = world::provider(scratch.path(), &[page.clone(), page]);
    world::refused(
        &world::invoke(scratch.path(), &provider, "declare", Some(&world::intent())),
        "concord.acceptance.closed",
    );
}
