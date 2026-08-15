#[path = "seat/spawn.rs"]
mod spawn;

use serde_json::Value;
use std::path::Path;
use std::process::Output;

#[test]
fn observation() {
    let fixture = tempfile::tempdir().expect("fixture");
    run(fixture.path(), &["domain", "bootstrap", "local"], &[]);
    run(fixture.path(), &["task", "start", "local", "seen"], &[]);
    let report = fixture.path().join("atoms.jsonl");
    let settings = [
        ("CONCORD_LOCUS_ENABLED", "true"),
        (
            "CONCORD_LOCUS_REPORT_FILE",
            report.to_str().expect("report path"),
        ),
        ("CODEX_THREAD_ID", "concord-thread"),
    ];
    let shown = run(fixture.path(), &["task", "show", "local/seen"], &settings);
    assert!(shown.status.success());
    assert_eq!(cycle(&read(&report), "task.show", 0), Value::Null);

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
