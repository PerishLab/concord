use concord_core::{Add, LandingProof, Root, Space};
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

struct Fixture {
    _temp: TempDir,
    space: Space,
}

impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().expect("temporary domain space");
        let root = Root::new(temp.path()).expect("canonical root");
        Self {
            space: Space::new(root),
            _temp: temp,
        }
    }

    fn member(&self, task: &str) -> (std::path::PathBuf, concord_core::TaskRef) {
        self.space
            .domain_init("local", true)
            .expect("initialize domain");
        self.space
            .task_start(&format!("local/{task}"), true)
            .expect("start task");
        let source = self.space.path().join("local/repo");
        std::fs::create_dir_all(&source).expect("create repository");
        git(&source, &["init", "-b", "main"]);
        git(&source, &["config", "user.name", "Concord Test"]);
        git(
            &source,
            &["config", "user.email", "concord@example.invalid"],
        );
        std::fs::write(source.join("README.md"), "# fixture\n").expect("write fixture");
        git(&source, &["add", "README.md"]);
        git(&source, &["commit", "-m", "init"]);
        self.space
            .member_add(
                Add {
                    task: &format!("local/{task}"),
                    name: "repo",
                    source: &source,
                    branch: None,
                    orphan: false,
                },
                true,
            )
            .expect("add member");
        (
            source,
            self.space
                .resolve(&format!("local/{task}"))
                .expect("resolve task"),
        )
    }
}

#[test]
fn preflight_proves_reachable_and_tree_equivalent_members() {
    let fixture = Fixture::new();
    let (source, task) = fixture.member("member");
    let member = task.member_path("repo");

    let reachable = task.preflight().expect("preflight reachable member");
    assert!(reachable.ok());
    let reachable_proof = reachable.members[0]
        .proof
        .as_ref()
        .expect("reachable proof");
    assert!(matches!(
        reachable_proof.landing,
        LandingProof::Reachable { .. }
    ));
    assert_eq!(reachable_proof.branch, "member");
    assert_eq!(
        reachable_proof.source_identity,
        reachable_proof.member_identity
    );
    assert!(reachable_proof.registered);
    assert!(reachable_proof.clean);

    std::fs::write(member.join("work.txt"), "in progress\n").expect("write task work");
    let dirty = task.preflight().expect("preflight dirty member");
    assert!(!dirty.ok());
    assert!(dirty.members[0].proof.is_none());
    assert!(dirty.faults.iter().any(|fault| fault.kind == "dirty"));

    git(&member, &["add", "work.txt"]);
    git(&member, &["commit", "-m", "task work"]);
    let unlanded = task.preflight().expect("preflight unlanded member");
    assert!(!unlanded.ok());
    assert!(unlanded.members[0].proof.is_none());
    assert!(
        unlanded
            .faults
            .iter()
            .any(|fault| fault.kind == "reachability")
    );
    assert!(
        fixture
            .space
            .member_remove("local/member", "repo", false)
            .is_err()
    );

    git(&source, &["merge", "--squash", "member"]);
    git(&source, &["commit", "-m", "land task work"]);
    let equivalent = task.preflight().expect("preflight tree-equivalent member");
    assert!(equivalent.ok());
    let equivalent_proof = equivalent.members[0]
        .proof
        .as_ref()
        .expect("tree-equivalent proof");
    match &equivalent_proof.landing {
        LandingProof::TreeEquivalent {
            member_tree,
            integration_tree,
            ..
        } => assert_eq!(member_tree, integration_tree),
        LandingProof::Reachable { .. } => panic!("expected tree-equivalent proof"),
    }
    fixture
        .space
        .member_remove("local/member", "repo", true)
        .expect("remove landed member");
}

#[test]
fn preflight_keeps_mismatched_and_missing_members_unproved() {
    let fixture = Fixture::new();
    let (source, task) = fixture.member("named");
    let member = task.member_path("repo");
    git(&member, &["branch", "-m", "wrong"]);
    let mismatch = task.preflight().expect("preflight branch mismatch");
    assert!(!mismatch.ok());
    assert!(mismatch.members[0].proof.is_none());
    assert!(mismatch.faults.iter().any(|fault| fault.kind == "branch"));
    git(&member, &["branch", "-m", "named"]);

    git(
        &source,
        &["worktree", "remove", member.to_str().expect("member path")],
    );
    let missing = task.preflight().expect("preflight missing member");
    assert!(!missing.ok());
    assert_eq!(missing.members.len(), 1);
    assert!(missing.members[0].proof.is_none());
    assert!(missing.faults.iter().any(|fault| fault.kind == "presence"));
}

fn git(root: &Path, args: &[&str]) {
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
        .args(args)
        .status()
        .expect("run git");
    assert!(status.success(), "git {} failed", args.join(" "));
}
