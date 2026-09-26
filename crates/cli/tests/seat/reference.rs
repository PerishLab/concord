use super::{git, human, raw, success};

#[test]
fn declared() {
    let fixture = tempfile::tempdir().expect("fixture");
    success(fixture.path(), &["domain", "bootstrap", "local"]);
    success(fixture.path(), &["task", "start", "local", "alpha"]);

    let set = success(
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
    assert_eq!(set["revision"], 1);
    let shown = success(fixture.path(), &["task", "show", "local/alpha"]);
    assert_eq!(shown["current"]["reference"]["kind"], "issue");
    assert_eq!(shown["current"]["reference"]["owner"], "PerishLab");
    assert_eq!(shown["current"]["reference"]["number"], 9);
    let stale = raw(
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
            "10",
            "--revision",
            "0",
        ],
    );
    assert!(!stale.status.success());
    assert!(String::from_utf8_lossy(&stale.stderr).contains("concord.task.stale"));

    let replaced = success(
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
            "10",
            "--revision",
            "1",
        ],
    );
    assert_eq!(replaced["revision"], 2);

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
            "2",
        ],
    );
    let member = success(
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
            "3",
        ],
    );
    assert_eq!(member["revision"], 4);
    let status = success(fixture.path(), &["member", "status", "local/alpha", "repo"]);
    assert_eq!(status["status"]["reference"]["kind"], "change");
    assert_eq!(status["status"]["reference"]["number"], 12);
    assert!(
        human(fixture.path(), &["member", "status", "local/alpha", "repo"])
            .contains("reference: change github/PerishLab/concord#12")
    );

    let refused = raw(
        fixture.path(),
        &[
            "member",
            "reference",
            "remove",
            "local/alpha",
            "repo",
            "--revision",
            "4",
        ],
    );
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("concord.apply.required"));
    assert_eq!(
        success(
            fixture.path(),
            &[
                "member",
                "reference",
                "remove",
                "local/alpha",
                "repo",
                "--revision",
                "4",
                "--apply",
            ],
        )["revision"],
        5
    );
    assert_eq!(
        success(
            fixture.path(),
            &[
                "task",
                "reference",
                "remove",
                "local/alpha",
                "--revision",
                "5",
                "--apply",
            ],
        )["revision"],
        6
    );
    let shown = success(fixture.path(), &["task", "show", "local/alpha"]);
    assert!(shown["current"].get("reference").is_none());
    let status = success(fixture.path(), &["member", "status", "local/alpha", "repo"]);
    assert!(status["status"].get("reference").is_none());
    assert_eq!(
        success(fixture.path(), &["audit"])["agreement"]["faults"],
        serde_json::json!([])
    );
}
