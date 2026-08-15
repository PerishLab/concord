#[path = "seat/spawn.rs"]
mod spawn;

use serde_json::Value;
use std::path::Path;
use std::process::{Command, Output};

#[test]
fn activity() {
    let fixture = tempfile::tempdir().expect("fixture");
    success(fixture.path(), &["domain", "bootstrap", "local"]);
    let started = success(fixture.path(), &["task", "start", "local", "alpha"]);
    let key = started["task"]["key"].as_i64().expect("Task key");

    let first = operator(
        fixture.path(),
        &["task", "show", "local/alpha"],
        "CODEX_THREAD_ID",
        "codex-one",
    );
    assert!(first.status.success());
    assert!(first.stderr.is_empty());

    let change = fixture.path().join("change.json");
    std::fs::write(
        &change,
        br#"{"version":1,"task":"local/alpha","revision":0,"edits":[{"op":"create","fact":{"role":"goal","body":"Prove warning scale"}}]}"#,
    )
    .expect("change envelope");
    let second = operator(
        fixture.path(),
        &[
            "task",
            "change",
            "--input",
            change.to_str().expect("change path"),
        ],
        "CLAUDE_CODE_SESSION_ID",
        "claude-one",
    );
    assert!(second.status.success());
    let changed: Value = serde_json::from_slice(&second.stdout).expect("change JSON");
    assert_eq!(changed["current"]["task"]["revision"], 1);
    let warning: Value = serde_json::from_slice(&second.stderr).expect("warning JSON");
    assert_eq!(
        warning["warning"]["code"],
        "concord.activity.concurrent_session"
    );
    assert_eq!(warning["warning"]["task"], "local/alpha");
    assert_eq!(warning["warning"]["current"]["agent"], "claude");
    assert_eq!(warning["warning"]["current"]["operation"], "task.change");
    assert_eq!(warning["warning"]["sessions"][0]["agent"], "codex");
    assert_eq!(warning["warning"]["sessions"][0]["session"], "codex-one");

    let current = success(fixture.path(), &["task", "show", "local/alpha"]);
    assert_eq!(current["current"]["task"]["revision"], 1);

    let human = spawn::concord(fixture.path())
        .args(["--root", fixture.path().to_str().expect("root path")])
        .args(["task", "show", "local/alpha"])
        .env("GROK_SESSION_ID", "grok-one")
        .output()
        .expect("run Concord as Grok operator");
    assert!(human.status.success());
    let warning = String::from_utf8_lossy(&human.stderr);
    assert!(warning.contains("has recent activity from another session; take care"));
    assert!(warning.contains("codex codex-one task.show at"));

    let repeated = spawn::concord(fixture.path())
        .args(["--root", fixture.path().to_str().expect("root path")])
        .args(["task", "show", "local/alpha"])
        .env("GROK_SESSION_ID", "grok-one")
        .output()
        .expect("repeat Concord as Grok operator");
    assert!(repeated.status.success());
    assert!(repeated.stderr.is_empty());

    std::fs::write(
        fixture.path().join(format!(".concord/activity/{key}.json")),
        b"not-json",
    )
    .expect("corrupt auxiliary ledger");
    let unaffected = operator(
        fixture.path(),
        &["task", "show", "local/alpha"],
        "CODEX_THREAD_ID",
        "codex-two",
    );
    assert!(unaffected.status.success());
    let current: Value = serde_json::from_slice(&unaffected.stdout).expect("primary result JSON");
    assert_eq!(current["current"]["task"]["revision"], 1);
    let warning: Value = serde_json::from_slice(&unaffected.stderr).expect("warning JSON");
    assert_eq!(warning["warning"]["code"], "concord.activity.unavailable");
    assert_eq!(
        warning["warning"]["details"]["code"],
        "concord.activity.invalid"
    );
}

#[test]
fn context() {
    let fixture = tempfile::tempdir().expect("fixture");
    success(fixture.path(), &["domain", "bootstrap", "local"]);
    success(fixture.path(), &["task", "start", "local", "alpha"]);
    let output = command(fixture.path())
        .args(["task", "show", "local/alpha"])
        .env("CLAUDE_CODE_SESSION_ID", "claude-one")
        .env("CODEX_THREAD_ID", "codex-one")
        .output()
        .expect("run Concord with ambiguous operator");
    assert!(output.status.success());
    let current: Value = serde_json::from_slice(&output.stdout).expect("primary result JSON");
    assert_eq!(current["current"]["task"]["revision"], 0);
    assert!(output.stderr.is_empty());

    let key = current["current"]["task"]["key"]
        .as_i64()
        .expect("Task key");
    let ledger: Value = serde_json::from_slice(
        &std::fs::read(fixture.path().join(format!(".concord/activity/{key}.json")))
            .expect("activity ledger"),
    )
    .expect("activity ledger JSON");
    let touch = ledger["touches"]
        .as_array()
        .expect("touches")
        .iter()
        .find(|touch| touch["operation"] == "task.show")
        .expect("show touch");
    assert!(touch.get("agent").is_none());
    assert!(touch.get("session").is_none());
    assert!(touch["time"].is_u64());
}

fn success(space: &Path, arguments: &[&str]) -> Value {
    let output = command(space)
        .args(arguments)
        .output()
        .expect("run Concord");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
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
    let mut command = spawn::concord(space);
    command.args(["--root", space.to_str().expect("root path"), "--json"]);
    command
}
