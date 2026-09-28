use concord_core::{Admission, Coordinate, IssueAttach, IssueProving, Seat, issue_delivery};
use plumb::guard::{Action, Descriptor};
use serde::Serialize;
use sha2::{Digest as _, Sha256};
use std::path::Path;
use std::process::Command;

const COMMIT: &str = "ded823859758b4f56a9ee5d5fca9d2ef72b62d13";
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

#[tokio::test(flavor = "current_thread")]
async fn exact() {
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
        .attach_issue(&IssueAttach {
            issue: issue.clone(),
            name: "kernel".into(),
            source: source.clone(),
            branch: None,
            claims: vec!["topic.md".into()],
            revision: 0,
        })
        .await
        .expect("attach Member");
    let member = temp.path().join(".issues/I_delivery/members/kernel");
    std::fs::write(member.join("topic.md"), "delivery\n").expect("change");
    git(&member, &["add", "topic.md"]);
    git(&member, &["commit", "-m", "Issue delivery"]);
    git(
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
            member: "kernel".into(),
            revision: 1,
        })
        .await
        .expect("prove Boundary");

    let snapshot = snapshot();
    let plan = issue_delivery::prepare(
        &estate,
        &issue_delivery::Request {
            issue,
            member: "kernel".into(),
            revision: 2,
            base: "main".into(),
            snapshot: snapshot.clone(),
            observed: 1,
            outcome: "Concord owns Issue-led delivery.".into(),
        },
    )
    .await
    .expect("prepare delivery");
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
    let ready = issue_delivery::revalidate(&estate, &plan, &snapshot, 2)
        .await
        .expect("revalidate delivery");
    assert_eq!(ready.preparation.candidate, plan.delivery.candidate);

    let mut drifted = snapshot;
    drifted.updated.push_str("-drift");
    let error = issue_delivery::revalidate(&estate, &plan, &drifted, 3)
        .await
        .expect_err("Issue drift");
    assert_eq!(error.code(), "concord.delivery.stale");
}

fn snapshot() -> plumb::delivery::Snapshot {
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

fn proof(tree: String) -> Descriptor {
    let mut proof = Descriptor {
        schema: plumb::guard::SCHEMA.into(),
        repository: "PerishLab/probe".into(),
        tree,
        plumb: format!("v0.55.0@{COMMIT}"),
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
