use concord_core::{Add, Root, Space};
use serde_json::Value;
use std::path::Path;
use std::process::{Command, Output};

fn git(root: &Path, arguments: &[&str]) {
    let mut command = Command::new("git");
    for name in [
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GIT_COMMON_DIR",
        "GIT_DIR",
        "GIT_INDEX_FILE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_PREFIX",
        "GIT_QUARANTINE_PATH",
        "GIT_WORK_TREE",
    ] {
        command.env_remove(name);
    }
    let status = command
        .arg("-C")
        .arg(root)
        .args(arguments)
        .status()
        .expect("run git");
    assert!(status.success(), "git {} failed", arguments.join(" "));
}

fn run(root: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_concord"))
        .arg("--root")
        .arg(root)
        .args(arguments)
        .output()
        .expect("run concord")
}

#[test]
fn preflight_prints_replayable_member_proof_and_keeps_faults_explicit() {
    let fixture = tempfile::tempdir().expect("fixture");
    let root = Root::new(fixture.path()).expect("canonical root");
    let space = Space::new(root);
    space.domain_init("local", true).expect("initialize domain");
    space.task_start("local/proof", true).expect("start task");
    let source = fixture.path().join("local/repo");
    std::fs::create_dir_all(&source).expect("create source");
    git(&source, &["init", "-b", "main"]);
    git(&source, &["config", "user.name", "Concord Test"]);
    git(
        &source,
        &["config", "user.email", "concord@example.invalid"],
    );
    std::fs::write(source.join("README.md"), "# fixture\n").expect("write source");
    git(&source, &["add", "README.md"]);
    git(&source, &["commit", "-m", "init"]);
    space
        .member_add(
            Add {
                task: "local/proof",
                name: "repo",
                source: &source,
                branch: None,
                orphan: false,
            },
            true,
        )
        .expect("add member");

    let output = run(
        fixture.path(),
        &["--json", "member", "preflight", "local/proof"],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout).expect("preflight json");
    assert_eq!(report["target"], "local/proof");
    assert_eq!(report["faults"], serde_json::json!([]));
    assert_eq!(report["members"][0]["name"], "repo");
    assert_eq!(report["members"][0]["expected_branch"], "proof");
    assert_eq!(report["members"][0]["proof"]["branch"], "proof");
    assert_eq!(
        report["members"][0]["proof"]["source_identity"],
        report["members"][0]["proof"]["member_identity"]
    );
    assert_eq!(report["members"][0]["proof"]["registered"], true);
    assert_eq!(
        report["members"][0]["proof"]["landing"]["relation"],
        "reachable"
    );
    assert!(
        report["members"][0]["proof"]["landing"]["member_head"]
            .as_str()
            .is_some_and(|head| !head.is_empty())
    );
    assert!(
        report["members"][0]["proof"]["landing"]["integration_head"]
            .as_str()
            .is_some_and(|head| !head.is_empty())
    );

    let human = run(fixture.path(), &["member", "preflight", "local/proof"]);
    assert!(human.status.success());
    let human = String::from_utf8(human.stdout).expect("human preflight");
    assert!(human.contains("member: repo"));
    assert!(human.contains("landed: reachable"));
    assert!(human.contains("ready to remove landed members"));

    let member = fixture.path().join("local/.tasks/proof").join("repo");
    std::fs::write(member.join("dirty.txt"), "dirty\n").expect("dirty member");
    let dirty = run(
        fixture.path(),
        &["--json", "member", "preflight", "local/proof"],
    );
    assert!(!dirty.status.success());
    let report: Value = serde_json::from_slice(&dirty.stdout).expect("dirty preflight json");
    assert!(report["members"][0].get("proof").is_none());
    assert_eq!(report["faults"][0]["kind"], "dirty");
}
