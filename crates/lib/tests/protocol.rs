use concord_core::{Add, Memory, Root, Space};
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

    fn init_task(&self, name: &str) {
        self.space
            .domain_init("local", true)
            .expect("initialize domain");
        self.space
            .task_start(&format!("local/{name}"), true)
            .expect("start task");
    }

    fn source(&self, name: &str) -> std::path::PathBuf {
        let path = self.space.path().join("local").join(name);
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
fn repo_less_task_memory_obeys_revisions_and_consent_boundaries() {
    let fixture = Fixture::new();
    fixture.init_task("memory");
    let task = fixture.space.resolve("local/memory").expect("resolve task");
    let memory = Memory::new(&task);

    assert!(memory.allocate("too-early").is_err());
    memory
        .init("# Current objective\n")
        .expect("initialize memory");
    let imported = fixture.space.path().join("probe");
    std::fs::write(&imported, "#!/bin/sh\n").expect("write resource");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&imported, std::fs::Permissions::from_mode(0o755))
            .expect("make resource executable");
    }
    let import = memory
        .preflight_import("evidence", &imported)
        .expect("preflight resource");
    assert_eq!(import.logical_bytes, 10);
    assert_eq!(import.entries, 1);
    assert!(import.required_bytes >= import.logical_bytes);
    assert!(import.available_bytes > import.required_bytes + import.reserve_bytes);
    let seat = memory
        .import("evidence", &imported)
        .expect("import resource");
    assert!(seat.join("probe").is_file());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(seat.join("probe"))
            .expect("resource metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o700);
    }
    let first = memory.read().expect("read memory");
    let second = memory
        .write(&first.revision, "# Current objective\n\nShip Concord.\n")
        .expect("replace memory");
    assert_ne!(first.revision, second.revision);
    assert!(memory.write(&first.revision, "stale").is_err());
    fixture
        .space
        .memory_remove("local/memory", true)
        .expect("remove exact unphased memory target");
    memory
        .init("# Current objective\n\nShip Concord.\n")
        .expect("reinitialize task memory");
    let second = memory.read().expect("read reinitialized memory");

    let (current, phase) = memory
        .settle(
            &second.revision,
            "# Phase goal\n\nProve revisions.\n",
            "# Current objective\n\nContinue.\n",
        )
        .expect("settle phase");
    assert!(phase.ends_with("PHASE-00.md"));
    assert_ne!(current.revision, second.revision);
    assert!(task.audit().expect("audit task").ok());
    assert!(fixture.space.task_finish("local/memory", false).is_err());

    let error = fixture
        .space
        .memory_remove("local/memory", true)
        .expect_err("retained phases must refuse memory removal");
    assert!(
        error
            .to_string()
            .contains("memory remove refuses task lineage with 1 retained phase(s)")
    );
    assert!(phase.is_file());
    assert!(memory.read().is_ok());
}

#[cfg(unix)]
#[test]
fn resource_import_preflight_refuses_symbolic_links_without_allocating_a_seat() {
    use std::os::unix::fs::symlink;

    let fixture = Fixture::new();
    fixture.init_task("linked-resource");
    let task = fixture
        .space
        .resolve("local/linked-resource")
        .expect("resolve task");
    let memory = Memory::new(&task);
    memory
        .init("# Current objective\n")
        .expect("initialize memory");

    let source = fixture.space.path().join("resource-source");
    std::fs::create_dir(&source).expect("create source");
    std::fs::write(source.join("held"), "evidence").expect("write source");
    symlink(source.join("held"), source.join("alias")).expect("link source");

    let error = memory
        .preflight_import("evidence", &source)
        .expect_err("symbolic links must be refused");
    assert!(error.to_string().contains("refuses symbolic link"));
    assert!(!memory.root().join("resources/evidence").exists());

    let linked_source = fixture.space.path().join("linked-source");
    symlink(source.join("held"), &linked_source).expect("link whole source");
    let error = memory
        .preflight_import("linked-evidence", &linked_source)
        .expect_err("a linked source must be refused");
    assert!(error.to_string().contains("refuses symbolic link"));
    assert!(!memory.root().join("resources/linked-evidence").exists());
}

#[test]
fn rehome_repairs_worktrees_and_makes_cross_domain_sources_explicit() {
    let fixture = Fixture::new();
    fixture.init_task("moving");
    fixture
        .space
        .domain_init("other", true)
        .expect("initialize target domain");
    let source = fixture.source("repo");
    fixture
        .space
        .member_add(
            Add {
                task: "local/moving",
                name: "repo",
                source: &source,
                branch: None,
                orphan: false,
                write: &[".".to_string()],
            },
            true,
        )
        .expect("add member");

    fixture
        .space
        .task_rehome("local/moving", "other", true)
        .expect("rehome task");
    let task = fixture
        .space
        .resolve("other/moving")
        .expect("resolve rehomed task");
    assert!(Path::new(&task.task().repo[0].source).is_absolute());
    assert!(task.audit().expect("audit rehomed task").ok());
}

#[test]
fn rename_preserves_the_member_branch_and_audit_reports_missing_seats() {
    let fixture = Fixture::new();
    fixture.init_task("before");
    let source = fixture.source("repo");
    fixture
        .space
        .member_add(
            Add {
                task: "local/before",
                name: "repo",
                source: &source,
                branch: None,
                orphan: false,
                write: &[".".to_string()],
            },
            true,
        )
        .expect("add member");
    fixture
        .space
        .task_rename("local/before", "after", true)
        .expect("rename task");
    let task = fixture
        .space
        .resolve("local/after")
        .expect("resolve renamed task");
    assert_eq!(task.task().repo[0].branch.as_deref(), Some("before"));
    assert!(task.audit().expect("audit renamed task").ok());

    git(
        &source,
        &[
            "worktree",
            "remove",
            task.member_path("repo").to_str().unwrap(),
        ],
    );
    let audit = task.audit().expect("audit missing seat");
    assert!(!audit.ok());
    assert_eq!(audit.faults[0].kind, "presence");
}

#[cfg(windows)]
#[test]
fn failed_member_add_rolls_back_the_created_branch() {
    let fixture = Fixture::new();
    fixture.init_task("rollback");
    let source = fixture.source("repo");
    let error = fixture
        .space
        .member_add(
            Add {
                task: "local/rollback",
                name: "CON",
                source: &source,
                branch: None,
                orphan: false,
                write: &[".".to_string()],
            },
            true,
        )
        .expect_err("reserved worktree path must fail");
    assert!(error.to_string().contains("git worktree add failed"));
    assert!(!branch_exists(&source, "rollback"));
    let task = fixture
        .space
        .resolve("local/rollback")
        .expect("resolve unchanged task");
    assert!(task.task().repo.is_empty());
    assert!(task.audit().expect("audit unchanged task").ok());
}

#[cfg(windows)]
fn branch_exists(root: &Path, branch: &str) -> bool {
    let reference = format!("refs/heads/{branch}");
    git_command(root)
        .args(["show-ref", "--verify", "--quiet", &reference])
        .status()
        .expect("inspect branch")
        .success()
}

fn git(root: &Path, args: &[&str]) {
    let status = git_command(root).args(args).status().expect("run git");
    assert!(status.success(), "git {} failed", args.join(" "));
}

fn git_command(root: &Path) -> Command {
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
    command.arg("-C").arg(root);
    command
}
