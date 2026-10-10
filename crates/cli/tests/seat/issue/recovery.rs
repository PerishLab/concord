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
