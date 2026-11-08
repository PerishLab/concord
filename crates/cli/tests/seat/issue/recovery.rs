use super::unix::{git, operator, success};
use serde_json::Value;
use std::path::Path;
use std::process::Command;

pub(super) fn run(space: &Path, source: &Path, pending: &Path) {
    let head = Command::new("git")
        .arg("-C")
        .arg(pending)
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("head");
    let head = String::from_utf8(head.stdout).expect("head");
    git(
        source,
        &["worktree", "remove", pending.to_str().expect("path")],
    );
    let recover = [
        "member",
        "recover",
        "PerishLab/concord#26",
        "--revision",
        "2",
        "--head",
        head.trim(),
    ];
    let refused = operator(space, &recover, "CODEX_THREAD_ID", "codex-one");
    assert!(!refused.status.success());
    let refusal = String::from_utf8_lossy(&refused.stderr);
    let line = refusal.lines().next_back().expect("refusal line");
    let error: Value = serde_json::from_str(line).expect("explicit apply refusal");
    assert_eq!(error["error"]["code"], "concord.apply.required");
    let arguments = recover.into_iter().chain(["--apply"]).collect::<Vec<_>>();
    assert_eq!(success(space, &arguments)["revision"], 2);
    assert_eq!(success(space, &arguments)["revision"], 2);
}

#[test]
fn detached() {
    use super::super::spawn;
    use super::unix::repository;
    use serde_json::json;

    let fixture = tempfile::tempdir().expect("fixture");
    let root = fixture.path();
    success(root, &["issue", "bootstrap"]);
    let source = repository(root, "PerishLab/concord");
    let provider = root.join("repository-provider");
    let reply = json!({"node": "R_concord", "coordinate": "PerishLab/concord", "branch": "main"});
    plumb::testing::executable(&provider, format!("#!/bin/sh\nprintf '%s\\n' '{reply}'\n"));
    let output = spawn::concord(root)
        .args(["--home", root.to_str().expect("root"), "--json"])
        .args(["integration", "register", "PerishLab/concord", "--path"])
        .arg(&source)
        .args(["--github-command"])
        .arg(&provider)
        .env("GIT_DIR", root.join("missing.git"))
        .env("GIT_WORK_TREE", root.join("wrong"))
        .env("GIT_INDEX_FILE", root.join("wrong-index"))
        .env("GIT_CONFIG_COUNT", "1")
        .env("GIT_CONFIG_KEY_0", "remote.origin.url")
        .env(
            "GIT_CONFIG_VALUE_0",
            "https://github.com/foreign/repository.git",
        )
        .env("GIT_NAMESPACE", "foreign")
        .output()
        .expect("register detached repository");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: Value = serde_json::from_slice(&output.stdout).expect("integration");
    assert_eq!(result["integration"]["repository"]["name"], "concord");
}
