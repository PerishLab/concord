#[cfg(unix)]
use super::{git, success};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(unix)]
use std::path::{Path, PathBuf};

#[cfg(unix)]
#[test]
fn observation() {
    let fixture = tempfile::tempdir().expect("fixture");
    success(fixture.path(), &["domain", "bootstrap", "local"]);
    success(fixture.path(), &["task", "start", "local", "alpha"]);
    success(fixture.path(), &["task", "start", "local", "beta"]);
    let unlinked = success(fixture.path(), &["task", "show", "local/beta", "--observe"]);
    assert_eq!(unlinked["observation"]["reason"], "reference");
    success(
        fixture.path(),
        &[
            "task",
            "reference",
            "set",
            "local/alpha",
            "--provider",
            "github",
            "--owner",
            "PerishLab",
            "--repository",
            "concord",
            "--number",
            "9",
            "--revision",
            "0",
        ],
    );
    let plain = success(fixture.path(), &["task", "show", "local/alpha"]);
    assert!(plain.get("observation").is_none());
    let absent = success(
        fixture.path(),
        &["task", "show", "local/alpha", "--observe"],
    );
    assert_eq!(absent["observation"]["availability"], "unavailable");
    assert_eq!(absent["observation"]["reason"], "command");

    let provider = tool(
        fixture.path(),
        "github",
        r#"if printf '%s' "$*" | grep -q pullRequest; then
printf '%s\n' '{"node":"PR_node","url":"https://github.com/PerishLab/concord/pull/12","state":"MERGED","updated_at":"2026-09-26T16:00:51Z","updated_at_epoch":1790438451}'
else
printf '%s\n' '{"node":"I_node","url":"https://github.com/PerishLab/concord/issues/9","state":"CLOSED","updated_at":"2026-09-26T16:01:14Z","updated_at_epoch":1790438474}'
fi"#,
    );
    let observed = success(
        fixture.path(),
        &[
            "task",
            "show",
            "local/alpha",
            "--observe",
            "--github-command",
            provider.to_str().expect("provider path"),
        ],
    );
    assert_eq!(observed["observation"]["availability"], "available");
    assert_eq!(observed["observation"]["node"], "I_node");
    assert_eq!(observed["observation"]["state"], "closed");
    assert!(observed["observation"].get("body").is_none());

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
    success(
        fixture.path(),
        &[
            "member",
            "attach",
            "local/alpha",
            "repo",
            "--source",
            source.to_str().expect("source path"),
            "--claim",
            "README.md",
            "--revision",
            "1",
        ],
    );
    success(
        fixture.path(),
        &[
            "member",
            "reference",
            "set",
            "local/alpha",
            "repo",
            "--provider",
            "github",
            "--owner",
            "PerishLab",
            "--repository",
            "concord",
            "--number",
            "12",
            "--revision",
            "2",
        ],
    );
    let member = success(
        fixture.path(),
        &[
            "member",
            "status",
            "local/alpha",
            "repo",
            "--observe",
            "--github-command",
            provider.to_str().expect("provider path"),
        ],
    );
    assert_eq!(member["observation"]["node"], "PR_node");
    assert_eq!(member["observation"]["state"], "merged");

    for (name, body, reason) in [
        ("missing", "printf 'null\\n'", "missing"),
        ("malformed", "printf 'not-json\\n'", "malformed"),
        ("provider", "exit 1", "provider"),
        ("timeout", "sleep 2", "timeout"),
    ] {
        let command = tool(fixture.path(), name, body);
        let reply = success(
            fixture.path(),
            &[
                "task",
                "show",
                "local/alpha",
                "--observe",
                "--github-command",
                command.to_str().expect("provider path"),
                "--observe-timeout",
                "1",
            ],
        );
        assert_eq!(reply["current"]["task"]["name"], "alpha");
        assert_eq!(reply["observation"]["availability"], "unavailable");
        assert_eq!(reply["observation"]["reason"], reason);
    }
}

#[cfg(unix)]
fn tool(root: &Path, name: &str, body: &str) -> PathBuf {
    let path = root.join(name);
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("provider command");
    let mut permissions = std::fs::metadata(&path)
        .expect("provider metadata")
        .permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(&path, permissions).expect("provider mode");
    path
}
