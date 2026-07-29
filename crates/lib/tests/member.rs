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
fn member_add_allows_one_source_in_distinct_tasks_and_branches() {
    let fixture = Fixture::new();
    fixture.start_task("first");
    fixture.start_task("second");
    let source = fixture.source();

    for name in ["first", "second"] {
        fixture
            .space
            .member_add(
                Add {
                    task: &format!("local/{name}"),
                    name: "repo",
                    source: &source,
                    branch: None,
                    orphan: false,
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
fn member_add_refuses_a_branch_owned_by_another_task() {
    let fixture = Fixture::new();
    fixture.start_task("owner");
    fixture.start_task("contender");
    let source = fixture.source();
    fixture
        .space
        .member_add(
            Add {
                task: "local/owner",
                name: "repo",
                source: &source,
                branch: None,
                orphan: false,
            },
            true,
        )
        .expect("add branch owner");

    for apply in [false, true] {
        let error = fixture
            .space
            .member_add(
                Add {
                    task: "local/contender",
                    name: "repo",
                    source: &source,
                    branch: Some("owner"),
                    orphan: false,
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
