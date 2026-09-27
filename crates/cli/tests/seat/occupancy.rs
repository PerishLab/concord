use serde_json::Value;
use std::path::Path;
use std::process::{Command, Output};

#[test]
fn occupancy() {
    let fixture = tempfile::tempdir().expect("fixture");
    success(fixture.path(), &["domain", "bootstrap", "local"]);
    let first = operator(
        fixture.path(),
        &["task", "start", "local", "alpha", "--legacy"],
        "CODEX_THREAD_ID",
        "codex-one",
    );
    assert!(first.status.success());
    assert!(first.stderr.is_empty());
    let disjoint = operator(
        fixture.path(),
        &["task", "start", "local", "other", "--legacy"],
        "GROK_SESSION_ID",
        "grok-one",
    );
    assert!(disjoint.status.success());
    assert!(disjoint.stderr.is_empty());

    let concurrent = operator(
        fixture.path(),
        &["task", "rename", "local/alpha", "beta", "--revision", "0"],
        "CLAUDE_CODE_SESSION_ID",
        "claude-one",
    );
    assert!(concurrent.status.success());
    let records = warnings(&concurrent);
    let warning = records
        .iter()
        .find(|value| value["warning"]["code"] == "concord.occupancy.concurrent_session")
        .expect("occupancy warning");
    assert_eq!(warning["warning"]["lease_seconds"], 7200);
    assert_eq!(warning["warning"]["current"]["session"], "claude-one");
    assert_eq!(
        warning["warning"]["conflicts"][0]["holder"]["session"],
        "codex-one"
    );
    assert_eq!(
        warning["warning"]["conflicts"][0]["subjects"][0]["kind"],
        "task"
    );

    let read = operator(
        fixture.path(),
        &["task", "show", "local/beta"],
        "GROK_SESSION_ID",
        "grok-read",
    );
    assert!(read.status.success());
    let ledger = fixture.path().join(".concord/occupancy/ledger.json");
    let before: Value =
        serde_json::from_slice(&std::fs::read(&ledger).expect("ledger")).expect("occupancy JSON");
    assert!(
        before["holders"]
            .as_array()
            .expect("holders")
            .iter()
            .all(|holder| holder["session"] != "grok-read")
    );

    let renewed = operator(
        fixture.path(),
        &[
            "task",
            "reference",
            "set",
            "local/beta",
            "--provider",
            "github",
            "--owner",
            "PerishLab",
            "--repository",
            "concord",
            "--number",
            "3",
            "--revision",
            "1",
        ],
        "CLAUDE_CODE_SESSION_ID",
        "claude-one",
    );
    assert!(renewed.status.success());
    let after: Value =
        serde_json::from_slice(&std::fs::read(&ledger).expect("ledger")).expect("occupancy JSON");
    assert_eq!(
        after["holders"]
            .as_array()
            .expect("holders")
            .iter()
            .filter(|holder| holder["session"] == "claude-one")
            .count(),
        1
    );

    std::fs::write(&ledger, b"not json").expect("corrupt occupancy");
    let unaffected = operator(
        fixture.path(),
        &[
            "task",
            "reference",
            "remove",
            "local/beta",
            "--revision",
            "2",
            "--apply",
        ],
        "CLAUDE_CODE_SESSION_ID",
        "claude-one",
    );
    assert!(unaffected.status.success());
    let primary: Value = serde_json::from_slice(&unaffected.stdout).expect("primary JSON");
    assert_eq!(primary["revision"], 3);
    assert!(warnings(&unaffected).iter().any(|value| {
        value["warning"]["code"] == "concord.occupancy.unavailable"
            && value["warning"]["details"]["code"] == "concord.occupancy.invalid"
    }));
}

#[test]
fn surfaces() {
    let fixture = tempfile::tempdir().expect("fixture");
    success(fixture.path(), &["domain", "bootstrap", "local"]);
    success(
        fixture.path(),
        &["task", "start", "local", "alpha", "--legacy"],
    );
    success(
        fixture.path(),
        &["task", "start", "local", "beta", "--legacy"],
    );
    let graph = operator(
        fixture.path(),
        &[
            "task",
            "dependency",
            "add",
            "local/alpha",
            "local/beta",
            "--weight",
            "required",
            "--revision",
            "0",
        ],
        "CODEX_THREAD_ID",
        "codex-graph",
    );
    assert!(graph.status.success());
    assert_eq!(
        holders(fixture.path())[0]["subjects"]
            .as_array()
            .expect("subjects")
            .iter()
            .map(|subject| subject["kind"].as_str().expect("kind"))
            .collect::<Vec<_>>(),
        vec!["graph", "graph"]
    );

    let failed = operator(
        fixture.path(),
        &["task", "rename", "local/alpha", "wrong", "--revision", "9"],
        "CLAUDE_CODE_SESSION_ID",
        "claude-failed",
    );
    assert!(!failed.status.success());
    assert!(
        holders(fixture.path())
            .iter()
            .all(|holder| holder["session"] != "claude-failed")
    );

    let source = fixture.path().join("source");
    std::fs::create_dir(&source).expect("source");
    git(&source, &["init", "-b", "main"]);
    git(&source, &["config", "user.name", "Concord Test"]);
    git(
        &source,
        &["config", "user.email", "concord@example.invalid"],
    );
    std::fs::write(source.join("README.md"), "fixture\n").expect("fixture");
    git(&source, &["add", "README.md"]);
    git(&source, &["commit", "-m", "fixture"]);
    let member = operator(
        fixture.path(),
        &[
            "member",
            "attach",
            "local/alpha",
            "repo",
            "--source",
            source.to_str().expect("source path"),
            "--claim",
            "crates",
            "--revision",
            "0",
        ],
        "GROK_SESSION_ID",
        "grok-member",
    );
    assert!(member.status.success());
    assert_eq!(
        holders(fixture.path())[0]["subjects"]
            .as_array()
            .expect("subjects")
            .iter()
            .map(|subject| subject["kind"].as_str().expect("kind"))
            .collect::<Vec<_>>(),
        vec!["task", "member"]
    );
}

fn holders(root: &Path) -> Vec<Value> {
    let ledger: Value = serde_json::from_slice(
        &std::fs::read(root.join(".concord/occupancy/ledger.json")).expect("ledger"),
    )
    .expect("occupancy JSON");
    ledger["holders"].as_array().expect("holders").clone()
}

fn warnings(output: &Output) -> Vec<Value> {
    String::from_utf8_lossy(&output.stderr)
        .lines()
        .map(|line| serde_json::from_str(line).expect("warning JSON"))
        .collect()
}

fn success(space: &Path, arguments: &[&str]) -> Value {
    let output = command(space)
        .args(arguments)
        .output()
        .expect("run Concord");
    assert!(output.status.success());
    serde_json::from_slice(&output.stdout).expect("Concord JSON")
}

fn operator(space: &Path, arguments: &[&str], variable: &str, session: &str) -> Output {
    command(space)
        .args(arguments)
        .env(variable, session)
        .output()
        .expect("run Concord as operator")
}

fn command(space: &Path) -> Command {
    let mut command = super::spawn::concord(space);
    command.args(["--root", space.to_str().expect("root path"), "--json"]);
    command
}

fn git(root: &Path, arguments: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(arguments)
        .status()
        .expect("run Git");
    assert!(status.success());
}
