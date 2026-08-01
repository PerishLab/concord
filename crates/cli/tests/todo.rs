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
