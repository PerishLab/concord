mod boundary;
mod completion;

use concord_core::authority::Plumb;
use concord_core::{
    Admission, Coordinate, Estate, IssueProving, Register, Repository, Seat, Start, issue_delivery,
};
use plumb::guard::{Action, Authority, Descriptor};
use serde::Serialize;
use sha2::{Digest as _, Sha256};
use std::path::Path;
use std::process::Command;

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

pub(super) struct Fixture {
    pub(super) estate: Estate,
    pub(super) issue: Coordinate,
    pub(super) _temp: tempfile::TempDir,
}

pub(super) fn released() -> Authority {
    Authority::released().expect("compiled Plumb authority")
}

pub(super) fn request(issue: &Coordinate) -> issue_delivery::Request {
    issue_delivery::Request {
        authority: issue_delivery::Mode::Plumb,
        issue: issue.clone(),
        revision: 2,
        base: "main".into(),
        snapshot: snapshot(),
        observed: 1,
        outcome: "Concord owns Issue-led delivery.".into(),
        acceptance: None,
    }
}

pub(super) async fn fixture(producer: &str, depot: &str) -> Fixture {
    let temp = tempfile::tempdir().expect("temporary Space");
    let source = temp.path().join("source");
    let remote = temp.path().join("remote.git");
    std::fs::create_dir(&source).expect("source");
    std::fs::create_dir(&remote).expect("remote");
    git(&remote, &["init", "--bare"]);
    git(&source, &["init", "-b", "main"]);
    git(&source, &["config", "user.name", "Concord Test"]);
    git(
        &source,
        &["config", "user.email", "concord@example.invalid"],
    );
    std::fs::write(source.join("README.md"), "fixture\n").expect("fixture");
    git(&source, &["add", "README.md"]);
    git(&source, &["commit", "-m", "fixture"]);
    git(
        &source,
        &["remote", "add", "origin", remote.to_str().expect("remote")],
    );
    git(&source, &["push", "-u", "origin", "main"]);
    git(
        &source,
        &[
            "remote",
            "set-url",
            "origin",
            "https://github.com/PerishLab/probe.git",
        ],
    );
    git(
        &source,
        &[
            "config",
            &format!("url.{}.insteadOf", remote.display()),
            "https://github.com/PerishLab/probe.git",
        ],
    );

    let estate = Seat::new(temp.path())
        .bootstrap()
        .await
        .expect("bootstrap estate");
    let issue = Coordinate::parse("PerishLab/probe#1").expect("coordinate");
    estate
        .admit(&Admission {
            node: "I_delivery".into(),
            coordinate: issue.clone(),
        })
        .await
        .expect("admit Issue");
    estate
        .register(&Register {
            node: "R_probe".into(),
            repository: Repository::parse("PerishLab/probe").expect("repository"),
            path: source.clone(),
        })
        .await
        .expect("register Integration");
    estate
        .start(&Start {
            issue: issue.clone(),
            node: "I_delivery".into(),
            stable: "R_probe".into(),
            kind: "Task".to_string(),
            claims: vec!["topic.md".into()],
            revision: 0,
        })
        .await
        .expect("attach Member");
    git(
        &source,
        &[
            "config",
            "--unset-all",
            &format!("url.{}.insteadOf", remote.display()),
        ],
    );
    let member = temp.path().join(".issues/I_delivery/worktree");
    std::fs::write(member.join("topic.md"), "delivery\n").expect("change");
    git(&member, &["add", "topic.md"]);
    git(&member, &["commit", "-m", "Issue delivery"]);
    let tree = text(&member, &["rev-parse", "HEAD^{tree}"]);
    let proof = proof(tree, producer, depot);
    let token = proof.encode().expect("proof");
    git(
        &member,
        &[
            "commit",
            "--amend",
            "-m",
            &format!("Issue delivery\n\n{} {token}", plumb::guard::TRAILER),
        ],
    );
    estate
        .prove_issue(&IssueProving {
            issue: issue.clone(),
            revision: 1,
        })
        .await
        .expect("prove Boundary");
    Fixture {
        estate,
        issue,
        _temp: temp,
    }
}

#[tokio::test(flavor = "current_thread")]
async fn exact() {
    let compiled = released();
    let Fixture {
        estate,
        issue,
        _temp,
    } = fixture(compiled.producer(), compiled.depot()).await;
    let snapshot = snapshot();
    let plan = issue_delivery::prepare::<Plumb>(&estate, &request(&issue))
        .await
        .expect("prepare delivery");
    let issue_delivery::Authority::Plumb { warrant, .. } = &plan.authority else {
        panic!("expected Guard authority");
    };
    assert_eq!(warrant.producer, compiled.producer());
    assert_eq!(warrant.depot, compiled.depot());
    assert_eq!(plan.delivery.issue, snapshot);
    assert!(
        plan.delivery
            .pull
            .body
            .starts_with("Refs PerishLab/probe#1")
    );
    for section in ["## Outcome", "## Change", "## Verification", "## Boundary"] {
        assert!(plan.delivery.pull.body.contains(section), "{section}");
    }
    assert!(!plan.delivery.pull.body.contains("Closes"));
    let decoded: issue_delivery::Plan =
        serde_json::from_slice(&serde_json::to_vec(&plan).expect("encode")).expect("decode");
    assert_eq!(decoded, plan);
    let boundary = serde_json::to_value(&plan.boundary).expect("boundary");
    let fields = boundary
        .as_object()
        .expect("boundary object")
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>();
    assert_eq!(fields, ["base", "claim", "head", "key", "plumb", "schema"]);
    assert!(
        plan.boundary
            .current(&plan.boundary.head, &plan.boundary.claim)
    );
    let ready = issue_delivery::revalidate::<Plumb>(&estate, &plan, &snapshot, 2)
        .await
        .expect("revalidate delivery");
    assert_eq!(ready.preparation.candidate, plan.delivery.candidate);

    let mut drifted = snapshot.clone();
    drifted.updated.push_str("-drift");
    let error = issue_delivery::revalidate::<Plumb>(&estate, &plan, &drifted, 3)
        .await
        .expect_err("Issue drift");
    assert_eq!(error.code(), "concord.delivery.stale");

    let member = _temp.path().join(".issues/I_delivery/worktree");
    std::fs::rename(&member, _temp.path().join("displaced")).expect("displace Member");
    let error = issue_delivery::revalidate::<Plumb>(&estate, &plan, &snapshot, 3)
        .await
        .expect_err("estate disagreement");
    assert_eq!(error.code(), "concord.audit.refused");
    let details = error.details().expect("agreement details");
    assert_eq!(
        details["agreement"]["faults"][0]["code"],
        "integration.worktree.missing"
    );
}

pub(super) fn snapshot() -> plumb::delivery::Snapshot {
    plumb::delivery::Snapshot {
        node: "I_delivery".into(),
        repository: "PerishLab/probe".into(),
        number: 1,
        url: "https://github.com/PerishLab/probe/issues/1".into(),
        title: "Issue delivery".into(),
        state: "OPEN".into(),
        kind: "Feature".into(),
        updated: "2026-09-28T00:00:00Z".into(),
        parent: None,
        sub_issues: Vec::new(),
        blocked_by: Vec::new(),
        blocking: Vec::new(),
    }
}

fn proof(tree: String, producer: &str, depot: &str) -> Descriptor {
    let mut proof = Descriptor {
        schema: plumb::guard::SCHEMA.into(),
        repository: "PerishLab/probe".into(),
        tree,
        plumb: producer.into(),
        depot: depot.into(),
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

fn git(root: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .status()
        .expect("git");
    assert!(status.success(), "git {args:?}");
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
