use concord_core::{Attach, Claiming, Proving, Seat, landing};
use plumb::guard::{Action, Descriptor};
use serde::Serialize;
use sha2::{Digest as _, Sha256};
use std::path::{Path, PathBuf};
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

struct Fixture {
    temp: tempfile::TempDir,
    source: PathBuf,
    member: PathBuf,
    guard: landing::Guard,
}

impl Fixture {
    async fn new() -> (Self, concord_core::Estate) {
        let temp = tempfile::tempdir().expect("temporary Space");
        let source = temp.path().join("source");
        let remote = temp.path().join("remote.git");
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
        let estate = Seat::new(temp.path())
            .bootstrap()
            .await
            .expect("bootstrap estate");
        estate.manage("local").await.expect("manage Domain");
        estate.start("local", "work").await.expect("start Task");
        estate
            .attach(&Attach {
                task: "local/work".into(),
                name: "repo".into(),
                source: source.clone(),
                branch: None,
                claims: vec!["topic.md".into()],
                revision: 0,
            })
            .await
            .expect("attach Member");
        let member = temp.path().join("local/.tasks/work/repo");
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
        let guard = landing::Guard {
            schema: plumb::guard::SCHEMA.into(),
            tree: proof.tree,
            digest: proof.digest,
        };
        (
            Self {
                temp,
                source,
                member,
                guard,
            },
            estate,
        )
    }

    fn request(&self) -> landing::Request {
        landing::Request {
            task: "local/work".into(),
            member: "repo".into(),
            revision: 2,
            base: "main".into(),
            title: "Deliver exact work".into(),
            body: "Refs PerishLab/probe#1".into(),
            guard: self.guard.clone(),
        }
    }
}

#[tokio::test(flavor = "current_thread")]
async fn exact() {
    let (fixture, estate) = Fixture::new().await;
    assert!(fixture.temp.path().is_dir());
    assert!(fixture.source.is_dir());
    let mut early = fixture.request();
    early.revision = 1;
    let error = landing::prepare(&estate, &early)
        .await
        .expect_err("Boundary required");
    assert_eq!(error.code(), "concord.landing.boundary");
    estate
        .prove(&Proving {
            task: "local/work".into(),
            member: "repo".into(),
            revision: 1,
        })
        .await
        .expect("prove Boundary");
    let plan = landing::prepare(&estate, &fixture.request())
        .await
        .expect("prepare landing");
    let encoded = serde_json::to_vec(&plan).expect("encode plan");
    let decoded: landing::Plan = serde_json::from_slice(&encoded).expect("decode plan");
    assert_eq!(decoded, plan);
    let ready = landing::revalidate(&estate, &plan)
        .await
        .expect("ready landing");
    assert_eq!(ready.plan, plan);
    assert_eq!(ready.schema, landing::READY);

    let mut changed = plan.clone();
    changed.landing.candidate = "5".repeat(40);
    let error = landing::revalidate(&estate, &changed)
        .await
        .expect_err("candidate drift");
    assert_eq!(error.code(), "concord.landing.stale");

    let mut narrative = plan.clone();
    narrative.landing.body = "Closes PerishLab/probe#1".into();
    let error = landing::revalidate(&estate, &narrative)
        .await
        .expect_err("narrative drift");
    assert_eq!(error.code(), "concord.landing.stale");

    let mut wrong = fixture.request();
    wrong.guard.digest = "6".repeat(64);
    let error = landing::prepare(&estate, &wrong)
        .await
        .expect_err("expected Guard drift");
    assert_eq!(error.code(), "concord.landing.guard");

    std::fs::write(fixture.member.join("dirty.txt"), "dirty\n").expect("dirty");
    let error = landing::prepare(&estate, &fixture.request())
        .await
        .expect_err("dirty Member");
    assert_eq!(error.code(), "concord.member.dirty");
    std::fs::remove_file(fixture.member.join("dirty.txt")).expect("clean");

    estate
        .claim(&Claiming {
            task: "local/work".into(),
            member: "repo".into(),
            claims: vec!["README.md".into()],
            revision: 2,
        })
        .await
        .expect("advance Task revision");
    let error = landing::revalidate(&estate, &plan)
        .await
        .expect_err("revision drift");
    assert_eq!(error.code(), "concord.task.stale");
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
