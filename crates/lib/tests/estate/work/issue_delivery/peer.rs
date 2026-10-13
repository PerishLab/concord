use concord_core::{
    Admission, Agreement, Coordinate, Estate, IssueProving, IssueRelease, Register, Repository,
    Seat, Start,
};
use std::path::Path;
use std::process::Command;

#[tokio::test(flavor = "current_thread")]
async fn peer() {
    let temp = tempfile::tempdir().expect("temporary Space");
    let source = temp.path().join("source");
    let remote = temp.path().join("remote.git");
    std::fs::create_dir(&source).expect("source directory");
    std::fs::create_dir(&remote).expect("remote directory");
    git(&remote, &["init", "--bare"]);
    git(&source, &["init", "-b", "main"]);
    git(&source, &["config", "user.name", "Concord Test"]);
    git(
        &source,
        &["config", "user.email", "concord@example.invalid"],
    );
    std::fs::write(source.join("README.md"), "fixture\n").expect("fixture file");
    git(&source, &["add", "README.md"]);
    git(&source, &["commit", "-m", "fixture"]);
    git(
        &source,
        &[
            "remote",
            "add",
            "origin",
            "https://github.com/PerishLab/concord.git",
        ],
    );
    git(
        &source,
        &[
            "config",
            &format!("url.{}.insteadOf", remote.display()),
            "https://github.com/PerishLab/concord.git",
        ],
    );
    git(&source, &["push", "-u", "origin", "main"]);

    let near = Seat::new(temp.path())
        .bootstrap()
        .await
        .expect("bootstrap estate");
    near.register(&Register {
        node: "R_concord".to_string(),
        repository: Repository::parse("PerishLab/concord").expect("repository"),
        path: source.clone(),
    })
    .await
    .expect("register Integration");
    let far = Seat::new(temp.path()).open().await.expect("second handle");
    agrees(&near.settled().await.expect("first read"));

    let issue = start(&far, "PerishLab/concord#41", "I_peer").await;
    agrees(&near.settled().await.expect("after peer start"));
    let late = Seat::new(temp.path()).open().await.expect("third handle");
    agrees(&late.settled().await.expect("late read"));

    let path = temp.path().join(".issues/I_peer/worktree");
    std::fs::create_dir(path.join("crates")).expect("claimed directory");
    std::fs::write(path.join("crates/note.md"), "peer\n").expect("peer delta");
    git(&path, &["add", "crates/note.md"]);
    git(&path, &["commit", "-m", "peer delta"]);
    far.prove_issue(&IssueProving {
        issue: issue.clone(),
        revision: 1,
    })
    .await
    .expect("prove peer");
    git(&source, &["merge", "--ff-only", "task/41"]);
    git(&source, &["push", "origin", "main"]);
    far.release_issue(&IssueRelease { issue, revision: 2 })
        .await
        .expect("release peer");
    agrees(&late.settled().await.expect("after peer release"));
    agrees(&near.settled().await.expect("after peer release"));

    start(&far, "PerishLab/concord#42", "I_lost").await;
    let lost = temp.path().join(".issues/I_lost/worktree");
    git(
        &source,
        &[
            "worktree",
            "remove",
            "--force",
            lost.to_str().expect("lost path"),
        ],
    );
    let report = near.settled().await.expect("after lost worktree");
    assert!(
        report
            .faults
            .iter()
            .any(|fault| fault.code == "integration.worktree.missing"
                && fault.subject.contains("I_lost")),
        "{:?}",
        report.faults
    );
}

async fn start(estate: &Estate, raw: &str, node: &str) -> Coordinate {
    let issue = Coordinate::parse(raw).expect("Issue coordinate");
    estate
        .admit(&Admission {
            node: node.to_string(),
            coordinate: issue.clone(),
        })
        .await
        .expect("attach Issue");
    estate
        .start(&Start {
            issue: issue.clone(),
            node: node.to_string(),
            stable: "R_concord".to_string(),
            kind: "Task".to_string(),
            claims: vec!["crates".to_string()],
            revision: 0,
        })
        .await
        .expect("start Member");
    issue
}

fn agrees(report: &Agreement) {
    assert!(report.agrees(), "{:?}", report.faults);
}

fn git(root: &Path, arguments: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(arguments)
        .status()
        .expect("run Git");
    assert!(status.success());
}
