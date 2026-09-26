#[path = "seat/spawn.rs"]
mod spawn;

use serde_json::{Value, json};
use std::path::Path;
use std::process::{Command, Output};

#[test]
fn exit() {
    let fixture = tempfile::tempdir().expect("fixture");
    success(fixture.path(), &["domain", "bootstrap", "local"]);
    let started = success(fixture.path(), &["task", "start", "local", "work"]);
    assert_eq!(started["task"]["revision"], 0);
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
            "work",
            "repo",
            "--source",
            source.to_str().expect("source path"),
            "--claim",
            "README.md",
            "--claim",
            "docs",
            "--revision",
            "0",
        ],
    );
    let claimed = success(
        fixture.path(),
        &[
            "member",
            "claim",
            "local/work",
            "repo",
            "--claim",
            "crates",
            "--revision",
            "1",
        ],
    );
    assert_eq!(
        claimed["member"]["claims"],
        json!(["README.md", "crates", "docs"])
    );
    let refused = raw(
        fixture.path(),
        &[
            "member",
            "narrow",
            "local/work",
            "repo",
            "--claim",
            "README.md",
            "--revision",
            "2",
        ],
    );
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("concord.apply.required"));
    let narrowed = success(
        fixture.path(),
        &[
            "member",
            "narrow",
            "local/work",
            "repo",
            "--claim",
            "README.md",
            "--revision",
            "2",
            "--apply",
        ],
    );
    assert_eq!(narrowed["member"]["claims"], json!(["README.md"]));

    let payload = fixture.path().join("bundle.txt");
    std::fs::write(&payload, "preserved\n").expect("artifact payload");
    success(
        fixture.path(),
        &[
            "artifact",
            "import",
            "local/work",
            "local-stack",
            "--source",
            payload.to_str().expect("payload path"),
        ],
    );
    success(
        fixture.path(),
        &["member", "prove", "local/work", "repo", "--revision", "3"],
    );
    let refused = raw(
        fixture.path(),
        &[
            "member",
            "retire",
            "local/work",
            "repo",
            "--artifacts",
            "*",
            "--revision",
            "4",
        ],
    );
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("concord.apply.required"));
    let missing = raw(
        fixture.path(),
        &[
            "member",
            "retire",
            "local/work",
            "repo",
            "--artifacts",
            "missing",
            "--revision",
            "4",
            "--apply",
        ],
    );
    assert!(!missing.status.success());
    assert!(String::from_utf8_lossy(&missing.stderr).contains("concord.artifact.absent"));
    let retired = success(
        fixture.path(),
        &[
            "member",
            "retire",
            "local/work",
            "repo",
            "--artifacts",
            "*",
            "--revision",
            "4",
            "--apply",
        ],
    );
    assert_eq!(retired["revision"], 5);
}

#[test]
fn repository() {
    let fixture = tempfile::tempdir().expect("fixture");
    success(fixture.path(), &["domain", "bootstrap", "local"]);
    let annotated = success(
        fixture.path(),
        &[
            "domain",
            "repository",
            "annotate",
            "local",
            "legacy",
            "--note",
            "RETIRED",
            "--revision",
            "0",
        ],
    );
    assert_eq!(annotated["revision"], 1);
    let refused = raw(
        fixture.path(),
        &[
            "domain",
            "repository",
            "retire",
            "local",
            "legacy",
            "--reason",
            "product was deleted",
            "--revision",
            "1",
        ],
    );
    assert!(!refused.status.success());
    assert!(
        String::from_utf8_lossy(&refused.stderr).contains("concord.apply.required"),
        "{}",
        String::from_utf8_lossy(&refused.stderr)
    );
    let active = success(
        fixture.path(),
        &["domain", "repository", "list", "--domain", "local"],
    );
    assert_eq!(active["repositories"][0]["name"], "legacy");
    let retired = success(
        fixture.path(),
        &[
            "domain",
            "repository",
            "retire",
            "local",
            "legacy",
            "--reason",
            "product was deleted",
            "--revision",
            "1",
            "--apply",
        ],
    );
    assert_eq!(retired["revision"], 2);
    assert_eq!(retired["repository"]["life"], "retired");
    assert_eq!(
        success(
            fixture.path(),
            &["domain", "repository", "list", "--domain", "local"],
        )["repositories"],
        json!([])
    );
    let retained = success(
        fixture.path(),
        &[
            "domain",
            "repository",
            "list",
            "--domain",
            "local",
            "--retired",
        ],
    );
    assert_eq!(retained["repositories"][0]["name"], "legacy");
    assert_eq!(retained["repositories"][0]["life"], "retired");
    assert_eq!(retained["repositories"][0]["reason"], "product was deleted");
}

fn success(space: &Path, arguments: &[&str]) -> Value {
    let output = raw(space, arguments);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("Concord JSON")
}

fn raw(space: &Path, arguments: &[&str]) -> Output {
    spawn::concord(space)
        .args(["--root", space.to_str().expect("root path"), "--json"])
        .args(arguments)
        .output()
        .expect("run Concord")
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
