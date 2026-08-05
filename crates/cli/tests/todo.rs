use concord_core::{Memory, Root, Space};
use serde_json::Value;
use std::path::Path;
use std::process::{Command, Output};

#[test]
fn task_todo_cli_projects_links_and_finish_handoffs() {
    let fixture = tempfile::tempdir().expect("fixture");
    success(fixture.path(), &["domain", "init", "local"]);
    success(fixture.path(), &["task", "start", "local/source"]);

    let added = success(
        fixture.path(),
        &["--json", "task", "todo", "add", "local/source", "future"],
    );
    let added: Value = serde_json::from_slice(&added.stdout).expect("add plan JSON");
    assert_eq!(added["operation"], "task.todo.add");
    assert_eq!(added["applied"], true);

    let shown = success(fixture.path(), &["--json", "task", "show", "local/source"]);
    let shown: Value = serde_json::from_slice(&shown.stdout).expect("task JSON");
    assert_eq!(shown["task"]["todo"][0], "future");

    let finish = success(
        fixture.path(),
        &["--json", "task", "finish", "local/source"],
    );
    let finish: Value = serde_json::from_slice(&finish.stdout).expect("finish plan JSON");
    assert_eq!(finish["actions"][0]["verb"], "handoff");

    let blocked = run(
        fixture.path(),
        &["task", "finish", "local/future", "--apply"],
    );
    assert!(!blocked.status.success());
    assert!(String::from_utf8_lossy(&blocked.stderr).contains("source"));

    success(
        fixture.path(),
        &[
            "task",
            "todo",
            "remove",
            "local/source",
            "future",
            "--apply",
        ],
    );
    success(
        fixture.path(),
        &["task", "finish", "local/future", "--apply"],
    );
}

#[test]
fn task_brief_cli_preserves_the_json_contract_and_human_cue() {
    let fixture = tempfile::tempdir().expect("fixture");
    let root = Root::new(fixture.path()).expect("canonical root");
    let space = Space::new(root);
    space.domain_init("local", true).expect("initialize domain");
    for name in ["alpha", "beta"] {
        space
            .task_start(&format!("local/{name}"), true)
            .expect("start task");
    }
    let alpha = space.resolve("local/alpha").expect("alpha");
    Memory::new(&alpha)
        .init("<!-- concord-memory:v1 -->\n# Brief\n\n## Goal\n\nShip.\n\n## Active constraints\n\nStay bounded.\n\n## Decisions in force\n\nReport facts.\n\n## Current focus\n\nChoose one owner.\n\n## Open questions\n\nNone.\n\n## Next step\n\nEnter its seat.\n")
        .expect("initialize memory");

    let output = run(
        fixture.path(),
        &["--json", "task", "brief", "--domain", "local"],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let body: Value = serde_json::from_slice(&output.stdout).expect("brief JSON");
    assert_eq!(body["schema"], "concord.task-brief:v1");
    assert_eq!(body["total"], 2);
    assert_eq!(body["tasks"][0]["identity"], "local/alpha");
    assert_eq!(body["tasks"][0]["memory"]["status"], "structured");
    assert_eq!(
        body["tasks"][0]["memory"]["sections"]["focus"]["text"],
        "Choose one owner."
    );
    assert_eq!(body["tasks"][1]["memory"]["status"], "absent");

    let human = run(fixture.path(), &["task", "brief", "--domain", "local"]);
    let human = String::from_utf8(human.stdout).expect("human output");
    assert!(human.contains("task brief: local (2/2"));
    assert!(human.contains("focus: Choose one owner."));
    assert!(human.contains("next: Enter its seat."));

    let refused = run(
        fixture.path(),
        &[
            "--json", "task", "brief", "--domain", "local", "--after", "missing",
        ],
    );
    assert!(!refused.status.success());
    let error: Value = serde_json::from_slice(&refused.stderr).expect("error JSON");
    assert_eq!(error["error"]["code"], "task.brief_cursor");
}

fn success(root: &Path, arguments: &[&str]) -> Output {
    let output = run(root, arguments);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn run(root: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_concord"))
        .arg("--root")
        .arg(root)
        .args(arguments)
        .output()
        .expect("run concord")
}
