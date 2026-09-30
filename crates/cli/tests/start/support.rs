use super::spawn;
use serde_json::{Value, json};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

pub fn fact() -> Value {
    json!({
        "node": "I_execution", "stable": "R_concord",
        "coordinate": "PerishLab/concord", "branch": "main", "number": 26,
        "url": "https://github.com/PerishLab/concord/issues/26",
        "state": "OPEN", "kind": "Feature", "leaves": 0, "truncated": false,
        "rulesets": [{
            "enforcement": "ACTIVE", "target": "BRANCH",
            "include": ["~DEFAULT_BRANCH"], "exclude": [], "bypass": 0,
            "truncated": false,
            "rules": ["DELETION", "NON_FAST_FORWARD", "PULL_REQUEST"],
        }],
        "labels": {"names": [], "truncated": false},
    })
}

pub fn tool(root: &Path, name: &str, reply: Value) -> PathBuf {
    let path = root.join(name);
    executable(&path, &format!("#!/bin/sh\nprintf '%s\\n' '{reply}'\n"));
    path
}

pub fn executable(path: &Path, body: &str) {
    std::fs::write(path, body).expect("provider command");
    let mut permissions = std::fs::metadata(path).expect("metadata").permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(path, permissions).expect("provider mode");
}

pub fn repository(root: &Path, coordinate: &str) -> PathBuf {
    let source = root.join("source");
    let remote = root.join("remote.git");
    std::fs::create_dir(&source).expect("source");
    std::fs::create_dir(&remote).expect("remote");
    git(&remote, &["init", "--bare"]);
    git(&source, &["init", "-b", "main"]);
    git(&source, &["config", "user.name", "Concord Test"]);
    git(
        &source,
        &["config", "user.email", "concord@example.invalid"],
    );
    std::fs::write(source.join("README.md"), "fixture\n").expect("fixture");
    git(&source, &["add", "README.md"]);
    git(&source, &["commit", "-m", "fixture"]);
    let github = format!("https://github.com/{coordinate}.git");
    git(&source, &["remote", "add", "origin", &github]);
    git(
        &source,
        &[
            "config",
            &format!("url.{}.insteadOf", remote.display()),
            &github,
        ],
    );
    git(&source, &["push", "-u", "origin", "main"]);
    source
}

pub fn git(root: &Path, arguments: &[&str]) {
    assert!(
        std::process::Command::new("git")
            .arg("-C")
            .arg(root)
            .args(arguments)
            .status()
            .expect("run Git")
            .success()
    );
}

pub fn success(space: &Path, arguments: &[&str]) -> Value {
    let output = spawn::concord(space)
        .args(["--root", space.to_str().expect("Space"), "--json"])
        .args(arguments)
        .output()
        .expect("run Concord");
    assert!(output.status.success());
    serde_json::from_slice(&output.stdout).expect("Concord JSON")
}
