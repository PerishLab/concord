use concord_core::{Add, Root, Space};
use std::path::{Path, PathBuf};
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
        let space = Space::new(root);
        space.domain_init("local", true).expect("initialize domain");
        Self { _temp: temp, space }
    }

    fn start_task(&self, name: &str) {
        self.space
            .task_start(&format!("local/{name}"), true)
            .expect("start task");
    }

    fn source(&self) -> PathBuf {
        let path = self.space.path().join("local/repo");
        std::fs::create_dir_all(&path).expect("create repository");
        git(&path, &["init", "-b", "main"]);
        git(&path, &["config", "user.name", "Concord Test"]);
        git(&path, &["config", "user.email", "concord@example.invalid"]);
        std::fs::write(path.join("README.md"), "# fixture\n").expect("write fixture");
        git(&path, &["add", "README.md"]);
        git(&path, &["commit", "-m", "init"]);
        path
    }
}

#[test]
fn member_add_allows_disjoint_claims_in_distinct_tasks_and_branches() {
    let fixture = Fixture::new();
    fixture.start_task("first");
    fixture.start_task("second");
    let source = fixture.source();

    for (name, write) in [("first", "crates/first"), ("second", "crates/second")] {
        let write = vec![write.to_string()];
        fixture
            .space
            .member_add(
                Add {
                    task: &format!("local/{name}"),
                    name: "repo",
                    source: &source,
                    branch: None,
                    orphan: false,
                    write: &write,
                },
                true,
            )
            .expect("add concurrent member");
        let task = fixture
            .space
            .resolve(&format!("local/{name}"))
            .expect("resolve concurrent task");
        assert!(task.member_path("repo").is_dir());
        assert!(task.audit().expect("audit concurrent task").ok());
    }
}

#[test]
fn member_add_refuses_overlapping_claims_for_one_git_identity() {
    let fixture = Fixture::new();
    fixture.start_task("owner");
    fixture.start_task("contender");
    let source = fixture.source();
    let owner = vec!["crates/lib".to_string()];
    fixture
        .space
        .member_add(
            Add {
                task: "local/owner",
                name: "repo",
                source: &source,
                branch: None,
                orphan: false,
                write: &owner,
            },
            true,
        )
        .expect("add claim owner");

    let contender = vec!["crates".to_string()];
    let error = fixture
        .space
        .member_add(
            Add {
                task: "local/contender",
                name: "repo",
                source: &source,
                branch: None,
                orphan: false,
                write: &contender,
            },
            true,
        )
        .expect_err("overlapping claim must be refused");
    assert!(error.to_string().contains("local/owner/repo"));
}

#[test]
fn boundary_proof_fails_closed_then_claim_expansion_opens_the_exact_path() {
    let fixture = Fixture::new();
    fixture.start_task("boundary");
    let source = fixture.source();
    let write = vec!["docs".to_string()];
    fixture
        .space
        .member_add(
            Add {
                task: "local/boundary",
                name: "repo",
                source: &source,
                branch: None,
                orphan: false,
                write: &write,
            },
            true,
        )
        .expect("add bounded member");
    let task = fixture
        .space
        .resolve("local/boundary")
        .expect("resolve task");
    let member = task.member_path("repo");
    std::fs::create_dir_all(member.join("crates/lib")).expect("create outside path");
    std::fs::write(member.join("crates/lib/lib.rs"), "pub fn held() {}\n")
        .expect("write outside path");
    git(&member, &["add", "crates/lib/lib.rs"]);
    git(&member, &["commit", "-m", "outside claim"]);

    let error = fixture
        .space
        .member_boundary("local/boundary", "repo")
        .expect_err("outside change must fail proof");
    assert!(error.to_string().contains("crates/lib/lib.rs"));

    fixture
        .space
        .member_claim("local/boundary", "repo", &["crates/lib".to_string()], true)
        .expect("expand member claim");
    fixture
        .space
        .member_boundary("local/boundary", "repo")
        .expect("prove expanded boundary");
    let task = fixture
        .space
        .resolve("local/boundary")
        .expect("resolve proved task");
    assert_eq!(task.task().repo[0].write, ["crates/lib", "docs"]);
    assert!(task.task().repo[0].boundary.is_some());
}

#[test]
fn claim_expansion_refuses_an_active_overlap_without_mutating_the_registry() {
    let fixture = Fixture::new();
    fixture.start_task("owner");
    fixture.start_task("contender");
    let source = fixture.source();
    for (task, path) in [("owner", "crates/lib"), ("contender", "docs")] {
        fixture
            .space
            .member_add(
                Add {
                    task: &format!("local/{task}"),
                    name: "repo",
                    source: &source,
                    branch: None,
                    orphan: false,
                    write: &[path.to_string()],
                },
                true,
            )
            .expect("add disjoint member");
    }
    let error = fixture
        .space
        .member_claim("local/contender", "repo", &["crates".to_string()], true)
        .expect_err("overlapping expansion must fail");
    assert!(error.to_string().contains("local/owner/repo"));
    let contender = fixture
        .space
        .resolve("local/contender")
        .expect("resolve contender");
    assert_eq!(contender.task().repo[0].write, ["docs"]);
}

#[test]
fn member_add_refuses_a_branch_owned_by_another_task() {
    let fixture = Fixture::new();
    fixture.start_task("owner");
    fixture.start_task("contender");
    let source = fixture.source();
    let write = vec!["docs".to_string()];
    fixture
        .space
        .member_add(
            Add {
                task: "local/owner",
                name: "repo",
                source: &source,
                branch: None,
                orphan: false,
                write: &write,
            },
            true,
        )
        .expect("add branch owner");

    for apply in [false, true] {
        let write = vec!["crates".to_string()];
        let error = fixture
            .space
            .member_add(
                Add {
                    task: "local/contender",
                    name: "repo",
                    source: &source,
                    branch: Some("owner"),
                    orphan: false,
                    write: &write,
                },
                apply,
            )
            .expect_err("owned target branch must be refused");
        assert_eq!(error.to_string(), "target branch already exists: owner");
        let task = fixture
            .space
            .resolve("local/contender")
            .expect("resolve unchanged task");
        assert!(task.task().repo.is_empty());
        assert!(!task.member_path("repo").exists());
        assert!(task.audit().expect("audit unchanged task").ok());
    }
    let owner = fixture
        .space
        .resolve("local/owner")
        .expect("resolve branch owner");
    assert!(owner.audit().expect("audit branch owner").ok());
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
