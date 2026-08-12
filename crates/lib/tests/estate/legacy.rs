use concord_core::{Boundary, Member, Registry, Repo, Root, Space, Task};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

const NEXT: &str = "<!-- concord-memory:v1 -->\n# Brief\n\n## Goal\n\nShip.\n\n## Active constraints\n\n- Preserve evidence.\n- Refuse drift.\n\n## Decisions in force\n\n## Current focus\n\nVerify.\n\n## Open questions\n\n## Next step\n\nCut over later.\n";
const PHASE: &str = "<!-- concord-phase:v1 -->\n# Epoch\n\n## Outcome\n\nStaged.\n\n## Decisions\n\n- Keep one transaction.\n- Preserve bullets whole.\n\n## Evidence\n\n## Carry-forward\n\n";

pub(super) fn space(root: &Path) -> Space {
    let domain = root.join("local");
    let tasks = domain.join(".tasks");
    let first = tasks.join("first");
    directory(&first);
    directory(&tasks.join("second"));

    let source = domain.join("repo");
    std::fs::create_dir(&source).expect("create source");
    git(&source, &["init", "-b", "main"]);
    git(&source, &["config", "user.name", "Concord Test"]);
    git(
        &source,
        &["config", "user.email", "concord@example.invalid"],
    );
    std::fs::write(source.join("README.md"), "fixture\n").expect("write fixture");
    git(&source, &["add", "README.md"]);
    git(&source, &["commit", "-m", "fixture"]);
    let worktree = first.join("repo");
    let text = worktree.to_string_lossy();
    git(&source, &["worktree", "add", "-b", "first", &text]);
    let head = output(&worktree, &["rev-parse", "HEAD"]);
    let write = vec!["crates".to_string()];

    let memory = first.join(".task");
    directory(&memory);
    file(&memory.join("MAIN.md"), NEXT);
    directory(&memory.join("phases"));
    file(&memory.join("phases/PHASE-00.md"), PHASE);
    directory(&memory.join("resources/proof"));
    file(&memory.join("resources/proof/payload"), "evidence\n");

    let mut first = task("first");
    first.todo.push("second".to_string());
    first.repo.push(Member {
        name: "repo".to_string(),
        source: "../repo".to_string(),
        branch: None,
        write: write.clone(),
        boundary: Some(Boundary {
            schema: plumb::boundary::SCHEMA.to_string(),
            plumb: "v0.18.11".to_string(),
            base: head.clone(),
            head,
            claim: digest(&write),
        }),
        extra: addition("legacy-member", "member-extra"),
    });
    first.extra = addition("legacy-task", "task-extra");
    let registry = Registry {
        version: 3,
        task: vec![first, task("second")],
        repo: vec![Repo {
            name: "repo".to_string(),
            note: Some("integration".to_string()),
            extra: addition("legacy-repository", "repository-extra"),
        }],
        extra: addition("legacy-domain", "domain-extra"),
    };
    file(
        &tasks.join("tasks.toml"),
        &toml::to_string_pretty(&registry).expect("serialize Registry"),
    );
    Space::new(Root::new(root).expect("canonical root"))
}

pub(super) fn cycle(root: &Path) -> Space {
    let tasks = root.join("local/.tasks");
    directory(&tasks.join("left"));
    directory(&tasks.join("right"));
    let mut left = task("left");
    left.todo.push("right".to_string());
    let mut right = task("right");
    right.todo.push("left".to_string());
    let registry = Registry {
        version: 3,
        task: vec![left, right],
        repo: Vec::new(),
        extra: BTreeMap::new(),
    };
    file(
        &tasks.join("tasks.toml"),
        &toml::to_string_pretty(&registry).expect("serialize Registry"),
    );
    Space::new(Root::new(root).expect("canonical root"))
}

fn git(repository: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(repository)
        .args(args)
        .status()
        .expect("run git");
    assert!(status.success(), "git {args:?}");
}

fn output(repository: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(repository)
        .args(args)
        .output()
        .expect("run git");
    assert!(output.status.success(), "git {args:?}");
    String::from_utf8(output.stdout)
        .expect("utf8 git output")
        .trim()
        .to_string()
}

fn digest(write: &[String]) -> String {
    let mut digest = Sha256::new();
    for path in write {
        digest.update(path.as_bytes());
        digest.update([0]);
    }
    format!("{:x}", digest.finalize())
}

fn addition(name: &str, value: &str) -> BTreeMap<String, toml::Value> {
    BTreeMap::from([(name.to_string(), toml::Value::String(value.to_string()))])
}

fn task(name: &str) -> Task {
    Task {
        name: name.to_string(),
        todo: Vec::new(),
        repo: Vec::new(),
        extra: BTreeMap::new(),
    }
}

fn directory(root: &Path) {
    std::fs::create_dir_all(root).expect("create private directory");
    mode(root, 0o700);
}

fn file(target: &Path, content: &str) {
    std::fs::write(target, content).expect("write private file");
    mode(target, 0o600);
}

#[cfg(unix)]
fn mode(target: &Path, value: u32) {
    use std::os::unix::fs::PermissionsExt;
    let permissions = std::fs::Permissions::from_mode(value);
    std::fs::set_permissions(target, permissions).expect("set private mode");
}

#[cfg(not(unix))]
fn mode(target: &Path, value: u32) {
    let _ = (target, value);
}
