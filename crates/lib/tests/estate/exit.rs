use concord_core::{Attach, Claiming, Import, Narrowing, Proving, Release, Retirement, Seat};
use std::{path::Path, process::Command};

#[tokio::test(flavor = "current_thread")]
async fn narrow() {
    let temp = tempfile::tempdir().expect("temporary Space");
    let source = temp.path().join("source");
    std::fs::create_dir(&source).expect("source directory");
    git(&source, &["init", "-b", "main"]);
    git(&source, &["config", "user.name", "Concord Test"]);
    git(
        &source,
        &["config", "user.email", "concord@example.invalid"],
    );
    std::fs::write(source.join("README.md"), "fixture\n").expect("fixture file");
    git(&source, &["add", "README.md"]);
    git(&source, &["commit", "-m", "fixture"]);

    let estate = Seat::new(temp.path())
        .bootstrap()
        .await
        .expect("bootstrap estate");
    estate.manage("local").await.expect("manage Domain");
    estate.start("local", "work").await.expect("start Task");
    let member = estate
        .attach(&Attach {
            task: "local/work".to_string(),
            name: "repo".to_string(),
            source: source.clone(),
            branch: None,
            claims: vec!["crates".to_string(), "docs".to_string()],
            revision: 0,
        })
        .await
        .expect("attach Member");
    assert_eq!(member.claims, vec!["crates", "docs"]);
    let path = temp.path().join("local/.tasks/work/repo");
    std::fs::create_dir(path.join("crates")).expect("crates directory");
    std::fs::write(path.join("crates/note.md"), "delta\n").expect("member delta");
    git(&path, &["add", "crates/note.md"]);
    git(&path, &["commit", "-m", "member delta"]);
    estate
        .prove(&Proving {
            task: "local/work".to_string(),
            member: "repo".to_string(),
            revision: 1,
        })
        .await
        .expect("prove Boundary");

    let error = estate
        .narrow(&Narrowing {
            task: "local/work".to_string(),
            member: "repo".to_string(),
            claims: vec!["docs".to_string()],
            revision: 2,
        })
        .await
        .expect_err("narrow must refuse an unprovable claim");
    assert_eq!(error.code(), "concord.boundary.outside");
    let held = estate
        .member_status("local/work", "repo")
        .await
        .expect("held status");
    assert_eq!(held.member.claims, vec!["crates", "docs"]);
    assert!(held.member.proof.is_some());

    let member = estate
        .narrow(&Narrowing {
            task: "local/work".to_string(),
            member: "repo".to_string(),
            claims: vec!["crates".to_string()],
            revision: 2,
        })
        .await
        .expect("narrow unused path");
    assert_eq!(member.claims, vec!["crates"]);
    assert!(member.proof.is_none());

    let error = estate
        .narrow(&Narrowing {
            task: "local/work".to_string(),
            member: "repo".to_string(),
            claims: vec!["crates".to_string()],
            revision: 3,
        })
        .await
        .expect_err("narrow must refuse an unchanged claim");
    assert_eq!(error.code(), "concord.claim.unchanged");

    let member = estate
        .claim(&Claiming {
            task: "local/work".to_string(),
            member: "repo".to_string(),
            claims: vec!["docs".to_string()],
            revision: 3,
        })
        .await
        .expect("claim still expands");
    assert_eq!(member.claims, vec!["crates", "docs"]);
}

#[tokio::test(flavor = "current_thread")]
async fn named() {
    let temp = tempfile::tempdir().expect("temporary Space");
    let source = temp.path().join("source");
    std::fs::create_dir(&source).expect("source directory");
    git(&source, &["init", "-b", "main"]);
    git(&source, &["config", "user.name", "Concord Test"]);
    git(
        &source,
        &["config", "user.email", "concord@example.invalid"],
    );
    std::fs::write(source.join("README.md"), "fixture\n").expect("fixture file");
    git(&source, &["add", "README.md"]);
    git(&source, &["commit", "-m", "fixture"]);

    let estate = Seat::new(temp.path())
        .bootstrap()
        .await
        .expect("bootstrap estate");
    estate.manage("local").await.expect("manage Domain");
    estate.start("local", "work").await.expect("start Task");
    estate
        .attach(&Attach {
            task: "work".to_string(),
            name: "repo".to_string(),
            source: source.clone(),
            branch: None,
            claims: vec!["crates".to_string(), "docs".to_string()],
            revision: 0,
        })
        .await
        .expect("attach Member");

    let path = temp.path().join("local/.tasks/work/repo");
    std::fs::create_dir(path.join("crates")).expect("crates directory");
    std::fs::write(path.join("crates/note.md"), "delta\n").expect("member delta");
    git(&path, &["add", "crates/note.md"]);
    git(&path, &["commit", "-m", "member delta"]);

    estate
        .prove(&Proving {
            task: "work".to_string(),
            member: "repo".to_string(),
            revision: 1,
        })
        .await
        .expect("prove Boundary by Task name");

    let member = estate
        .narrow(&Narrowing {
            task: "work".to_string(),
            member: "repo".to_string(),
            claims: vec!["crates".to_string()],
            revision: 2,
        })
        .await
        .expect("narrow by Task name");
    assert_eq!(member.claims, vec!["crates"]);

    let member = estate
        .claim(&Claiming {
            task: "work".to_string(),
            member: "repo".to_string(),
            claims: vec!["docs".to_string()],
            revision: 3,
        })
        .await
        .expect("claim by Task name");
    assert_eq!(member.claims, vec!["crates", "docs"]);
}

#[tokio::test(flavor = "current_thread")]
async fn retire() {
    let temp = tempfile::tempdir().expect("temporary Space");
    let source = temp.path().join("source");
    std::fs::create_dir(&source).expect("source directory");
    git(&source, &["init", "-b", "main"]);
    git(&source, &["config", "user.name", "Concord Test"]);
    git(
        &source,
        &["config", "user.email", "concord@example.invalid"],
    );
    std::fs::write(source.join("README.md"), "fixture\n").expect("fixture file");
    git(&source, &["add", "README.md"]);
    git(&source, &["commit", "-m", "fixture"]);

    let estate = Seat::new(temp.path())
        .bootstrap()
        .await
        .expect("bootstrap estate");
    estate.manage("local").await.expect("manage Domain");
    estate.start("local", "work").await.expect("start Task");
    estate
        .attach(&Attach {
            task: "local/work".to_string(),
            name: "repo".to_string(),
            source: source.clone(),
            branch: None,
            claims: vec!["crates".to_string()],
            revision: 0,
        })
        .await
        .expect("attach Member");
    let path = temp.path().join("local/.tasks/work/repo");
    std::fs::create_dir(path.join("crates")).expect("crates directory");
    std::fs::write(path.join("crates/note.md"), "delta\n").expect("member delta");
    git(&path, &["add", "crates/note.md"]);
    git(&path, &["commit", "-m", "member delta"]);
    estate
        .prove(&Proving {
            task: "local/work".to_string(),
            member: "repo".to_string(),
            revision: 1,
        })
        .await
        .expect("prove Boundary");

    let error = estate
        .release(&Release {
            task: "local/work".to_string(),
            member: "repo".to_string(),
            revision: 2,
        })
        .await
        .expect_err("release must still refuse an unlanded Member");
    assert_eq!(error.code(), "concord.member.unlanded");

    let error = estate
        .retire(&Retirement {
            task: "local/work".to_string(),
            member: "repo".to_string(),
            artifacts: vec!["*".to_string()],
            revision: 2,
        })
        .await
        .expect_err("retire must refuse when no Artifact matches");
    assert_eq!(error.code(), "concord.artifact.absent");
    assert!(path.join(".git").is_file());

    let payload = temp.path().join("bundle.txt");
    std::fs::write(&payload, "preserved\n").expect("artifact payload");
    estate
        .import(&Import {
            task: "local/work".to_string(),
            name: "local-stack".to_string(),
            source: payload,
        })
        .await
        .expect("import Artifact");

    let error = estate
        .retire(&Retirement {
            task: "local/work".to_string(),
            member: "repo".to_string(),
            artifacts: vec!["other".to_string()],
            revision: 2,
        })
        .await
        .expect_err("retire must refuse a name that matches nothing");
    assert_eq!(error.code(), "concord.artifact.absent");

    assert_eq!(
        estate
            .retire(&Retirement {
                task: "local/work".to_string(),
                member: "repo".to_string(),
                artifacts: vec!["local-stack".to_string()],
                revision: 2,
            })
            .await
            .expect("retire against a named Artifact"),
        3
    );
    assert!(estate.worktrees().await.expect("Worktrees").is_empty());
    assert!(!path.exists());
}

fn git(root: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .status()
        .expect("run git");
    assert!(status.success(), "git {args:?}");
}
