use plumb::guard::{Action, Descriptor};
use serde::Serialize;
use serde_json::json;
use sha2::{Digest as _, Sha256};
use std::path::Path;
use std::process::Command;

const COMMIT: &str = "891038814d7365de0fcfad9781864f623a8e728f";
const DEPOT: &str = "da911e1a1ae27c87f4ef8d9b7f638fcedf222d669798d075aafcaaab045250af";

#[derive(Serialize)]
struct Claim<'a> {
    schema: &'a str,
    repository: &'a str,
    tree: &'a str,
    plumb: &'a str,
    depot: &'a str,
    platform: &'a str,
    actions: &'a [Action],
}

#[test]
fn cycle() {
    let fixture = tempfile::tempdir().expect("fixture");
    let root = fixture.path();
    super::success(root, &["domain", "bootstrap", "local"]);
    super::success(root, &["task", "start", "local", "work", "--legacy"]);
    let source = root.join("source");
    let remote = root.join("remote.git");
    std::fs::create_dir(&source).expect("source");
    std::fs::create_dir(&remote).expect("remote");
    super::git(&remote, &["init", "--bare"]);
    super::git(&source, &["init", "-b", "main"]);
    super::git(&source, &["config", "user.name", "Concord Test"]);
    super::git(
        &source,
        &["config", "user.email", "concord@example.invalid"],
    );
    std::fs::write(source.join("README.md"), "fixture\n").expect("fixture");
    super::git(&source, &["add", "README.md"]);
    super::git(&source, &["commit", "-m", "fixture"]);
    super::git(
        &source,
        &[
            "remote",
            "add",
            "origin",
            remote.to_str().expect("remote path"),
        ],
    );
    super::git(&source, &["push", "-u", "origin", "main"]);
    super::success(
        root,
        &[
            "member",
            "attach",
            "local/work",
            "repo",
            "--source",
            source.to_str().expect("source path"),
            "--claim",
            "topic.md",
            "--revision",
            "0",
        ],
    );
    let member = root.join("local/.tasks/work/repo");
    std::fs::write(member.join("topic.md"), "work\n").expect("work");
    super::git(&member, &["add", "topic.md"]);
    super::git(&member, &["commit", "-m", "Add exact work"]);
    super::git(
        &source,
        &[
            "remote",
            "set-url",
            "origin",
            "https://github.com/PerishLab/probe.git",
        ],
    );
    let tree = text(&member, &["rev-parse", "HEAD^{tree}"]);
    let proof = proof(tree);
    let token = proof.encode().expect("proof");
    let message = format!("Add exact work\n\n{} {token}", plumb::guard::TRAILER);
    super::git(&member, &["commit", "--amend", "-m", &message]);
    super::success(
        root,
        &["member", "prove", "local/work", "repo", "--revision", "1"],
    );
    let prepared = super::success(
        root,
        &[
            "member",
            "landing",
            "prepare",
            "local/work",
            "repo",
            "--title",
            "Deliver exact work",
            "--body",
            "Refs PerishLab/probe#1",
            "--guard-schema",
            &proof.schema,
            "--guard-tree",
            &proof.tree,
            "--guard-digest",
            &proof.digest,
            "--revision",
            "2",
        ],
    );
    assert_eq!(prepared["version"], 1);
    assert_eq!(prepared["plan"]["schema"], "concord.member-landing/v1");
    assert_eq!(prepared["plan"]["task"], "local/work");
    let ready = super::pipe(
        root,
        &["member", "landing", "ready", "local/work", "repo"],
        prepared.clone(),
    );
    assert_eq!(ready["version"], 1);
    assert_eq!(ready["ready"]["schema"], "concord.member-ready/v1");
    assert_eq!(ready["ready"]["plan"], prepared["plan"]);
    assert_eq!(ready["ready"]["plan"]["member"]["name"], json!("repo"));
}

fn proof(tree: String) -> Descriptor {
    let mut proof = Descriptor {
        schema: plumb::guard::SCHEMA.into(),
        repository: "PerishLab/probe".into(),
        tree,
        plumb: format!("v0.53.0@{COMMIT}"),
        depot: DEPOT.into(),
        platform: plumb::config::platform(),
        actions: vec![Action {
            name: "guard/test".into(),
            input: "3".repeat(64),
            world: "4".repeat(64),
        }],
        digest: String::new(),
    };
    let claim = Claim {
        schema: &proof.schema,
        repository: &proof.repository,
        tree: &proof.tree,
        plumb: &proof.plumb,
        depot: &proof.depot,
        platform: &proof.platform,
        actions: &proof.actions,
    };
    proof.digest = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&claim).expect("claim"))
    );
    proof
}

fn text(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .expect("git");
    assert!(output.status.success());
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}
