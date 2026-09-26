#[path = "seat/spawn.rs"]
mod spawn;

use serde_json::Value;
use std::path::Path;
use std::process::{Command, Output};

#[test]
fn observation() {
    let fixture = tempfile::tempdir().expect("fixture");
    run(fixture.path(), &["domain", "bootstrap", "local"], &[]);
    run(fixture.path(), &["task", "start", "local", "seen"], &[]);
    run(
        fixture.path(),
        &[
            "task",
            "reference",
            "set",
            "local/seen",
            "--provider",
            "github",
            "--owner",
            "PerishLab",
            "--repository",
            "concord",
            "--number",
            "11",
            "--revision",
            "0",
        ],
        &[],
    );
    let source = fixture.path().join("source");
    std::fs::create_dir(&source).expect("source directory");
    git(&source, &["init", "-b", "main"]);
    git(&source, &["config", "user.name", "Concord Test"]);
    git(
        &source,
        &["config", "user.email", "concord@example.invalid"],
    );
    std::fs::write(source.join("README.md"), "fixture\n").expect("fixture file");
    git(&source, &["add", "README.md"]);
    git(&source, &["commit", "-m", "fixture"]);
    run(
        fixture.path(),
        &[
            "member",
            "attach",
            "local/seen",
            "repo",
            "--source",
            source.to_str().expect("source path"),
            "--claim",
            "README.md",
            "--revision",
            "1",
        ],
        &[],
    );
    run(
        fixture.path(),
        &[
            "member",
            "reference",
            "set",
            "local/seen",
            "repo",
            "--provider",
            "github",
            "--owner",
            "PerishLab",
            "--repository",
            "concord",
            "--number",
            "13",
            "--revision",
            "2",
        ],
        &[],
    );
    let report = fixture.path().join("atoms.jsonl");
    let settings = [
        ("CONCORD_LOCUS_ENABLED", "true"),
        (
            "CONCORD_LOCUS_REPORT_FILE",
            report.to_str().expect("report path"),
        ),
        ("CODEX_THREAD_ID", "concord-thread"),
    ];
    let shown = run(
        fixture.path(),
        &["task", "show", "local/seen", "--observe"],
        &settings,
    );
    assert!(shown.status.success());
    let atoms = read(&report);
    assert_eq!(cycle(&atoms, "task.show", 0), Value::Null);
    projection(&atoms, "task.current", "issue", "task.show");

    std::fs::remove_file(&report).expect("remove report");
    let member = run(
        fixture.path(),
        &["member", "status", "local/seen", "repo", "--observe"],
        &settings,
    );
    assert!(member.status.success());
    let atoms = read(&report);
    assert_eq!(cycle(&atoms, "member.status", 0), Value::Null);
    projection(&atoms, "member.status", "change", "member.status");

    std::fs::remove_file(&report).expect("remove report");
    let listed = run(fixture.path(), &["task", "list"], &settings);
    assert!(listed.status.success());
    assert_eq!(cycle(&read(&report), "task.list", 0), Value::Null);

    std::fs::remove_file(&report).expect("remove report");
    let missing = run(
        fixture.path(),
        &["task", "show", "local/missing"],
        &settings,
    );
    assert!(!missing.status.success());
    assert_eq!(
        cycle(&read(&report), "task.show", 1),
        Value::String("concord.task.absent".to_string())
    );
}

fn projection(atoms: &[Value], shape: &str, kind: &str, command: &str) {
    let event = atoms
        .iter()
        .find(|atom| atom["payload"]["event"] == "provider.projection")
        .expect("provider projection");
    assert_eq!(event["payload"]["shape"], shape);
    assert_eq!(event["payload"]["provider"], "github");
    assert_eq!(event["payload"]["kind"], kind);
    assert_eq!(event["payload"]["outcome"], "unavailable");
    assert!(event["payload"]["duration_ms"].is_number());
    assert_eq!(event["context"]["concord.command"], command);
    for absent in [
        "title",
        "body",
        "url",
        "repository",
        "number",
        "reply",
        "credential",
    ] {
        assert!(event["payload"].get(absent).is_none());
    }
}

fn run(root: &Path, arguments: &[&str], environment: &[(&str, &str)]) -> Output {
    let mut command = spawn::concord(root);
    command
        .args(["--root", root.to_str().expect("root path")])
        .args(arguments);
    for (name, value) in environment {
        command.env(name, value);
    }
    command.output().expect("run Concord")
}

fn read(path: &Path) -> Vec<Value> {
    std::fs::read_to_string(path)
        .expect("read atoms")
        .lines()
        .map(|line| serde_json::from_str(line).expect("atom JSON"))
        .collect()
}

fn cycle(atoms: &[Value], command: &str, code: i64) -> Value {
    let start = atoms
        .iter()
        .find(|atom| atom["payload"]["event"] == "cli.start")
        .expect("cli start");
    let finish = atoms
        .iter()
        .find(|atom| atom["payload"]["event"] == "cli.finish")
        .expect("cli finish");
    assert_eq!(start["context"]["concord.command"], command);
    assert_eq!(finish["context"]["concord.command"], command);
    assert_eq!(finish["payload"]["code"], code);
    assert_eq!(start["context"]["locus.trace"], "concord-thread");
    for atom in atoms.iter().filter(|atom| atom["source"].is_object()) {
        assert_eq!(atom["context"]["concord.command"], Value::Null);
        assert_eq!(atom["context"]["concord.fault"], Value::Null);
    }
    finish["context"]["concord.fault"].clone()
}

fn git(root: &Path, arguments: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(arguments)
        .status()
        .expect("run git");
    assert!(status.success(), "git {arguments:?}");
}
