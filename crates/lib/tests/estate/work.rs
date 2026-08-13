use concord_core::{Attach, Claiming, Finish, Life, Proving, Rehome, Release, Rename, Seat};
use std::path::Path;
use std::process::Command;

#[tokio::test(flavor = "current_thread")]
async fn member() {
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
    let attach = Attach {
        task: "work".to_string(),
        name: "repo".to_string(),
        source: source.clone(),
        branch: None,
        claims: vec!["crates".to_string()],
        revision: 0,
    };
    let member = estate.attach(&attach).await.expect("attach Member");
    assert_eq!(member.task, "local/work");
    assert_eq!(member.branch, "work");
    assert_eq!(member.claims, vec!["crates"]);
    assert!(temp.path().join("local/.tasks/work/repo/.git").is_file());

    let finish = Finish {
        task: "local/work".to_string(),
        revision: 1,
        graph: 0,
        reason: "blocked".to_string(),
    };
    let error = match estate.finish(&finish).await {
        Ok(_) => panic!("live Member must block finish"),
        Err(error) => error,
    };
    assert_eq!(error.code(), "concord.task.members");

    let claiming = Claiming {
        task: "local/work".to_string(),
        member: "repo".to_string(),
        claims: vec!["docs".to_string()],
        revision: 1,
    };
    let member = estate.claim(&claiming).await.expect("expand Claim");
    assert_eq!(member.claims, vec!["crates", "docs"]);
    estate
        .start("local", "other")
        .await
        .expect("start peer Task");
    let error = estate
        .attach(&Attach {
            task: "local/other".to_string(),
            name: "peer".to_string(),
            source: source.clone(),
            branch: None,
            claims: vec!["docs/guide".to_string()],
            revision: 0,
        })
        .await
        .expect_err("overlapping Claim across Tasks must refuse");
    assert_eq!(error.code(), "concord.claim.overlap");
    assert!(!temp.path().join("local/.tasks/other").exists());
    let proving = Proving {
        task: "local/work".to_string(),
        member: "repo".to_string(),
        revision: 2,
    };
    let member = estate.prove(&proving).await.expect("prove Boundary");
    assert!(member.proof.is_some());

    let claiming = Claiming {
        task: "local/work".to_string(),
        member: "repo".to_string(),
        claims: vec!["README.md".to_string()],
        revision: 3,
    };
    let member = estate.claim(&claiming).await.expect("invalidate Boundary");
    assert!(member.proof.is_none());

    let proving = Proving {
        task: "local/work".to_string(),
        member: "repo".to_string(),
        revision: 4,
    };
    estate.prove(&proving).await.expect("renew Boundary");
    let release = Release {
        task: "local/work".to_string(),
        member: "repo".to_string(),
        revision: 5,
    };
    assert_eq!(estate.release(&release).await.expect("release Member"), 6);
    assert!(estate.worktrees().await.expect("Worktrees").is_empty());
    assert!(!temp.path().join("local/.tasks/work/repo").exists());

    let finish = Finish {
        task: "local/work".to_string(),
        revision: 6,
        graph: 0,
        reason: "completed".to_string(),
    };
    let retired = estate.finish(&finish).await.expect("retire Task");
    assert_eq!(retired.life, Life::Retired);
}

#[tokio::test(flavor = "current_thread")]
async fn motion() {
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
    estate.manage("local").await.expect("manage local");
    estate.manage("other").await.expect("manage other");
    estate.start("local", "before").await.expect("start Task");
    estate
        .attach(&Attach {
            task: "local/before".to_string(),
            name: "repo".to_string(),
            source: source.clone(),
            branch: None,
            claims: vec!["crates".to_string()],
            revision: 0,
        })
        .await
        .expect("attach Member");
    estate
        .rename(&Rename {
            task: "local/before".to_string(),
            name: "after".to_string(),
            revision: 1,
        })
        .await
        .expect("rename Task");
    let renamed = temp.path().join("local/.tasks/after/repo");
    assert!(renamed.join(".git").is_file());
    assert!(registered(&source, &renamed));
    estate
        .rehome(&Rehome {
            task: "local/after".to_string(),
            domain: "other".to_string(),
            revision: 2,
        })
        .await
        .expect("rehome Task");
    let moved = temp.path().join("other/.tasks/after/repo");
    assert!(moved.join(".git").is_file());
    assert!(registered(&source, &moved));
    let members = estate.worktrees().await.expect("read Members");
    assert_eq!(members[0].task, "other/after");
    assert_eq!(members[0].branch, "before");
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

fn registered(source: &Path, member: &Path) -> bool {
    let member = member.canonicalize().expect("canonical Member");
    let output = Command::new("git")
        .arg("-C")
        .arg(source)
        .args(["worktree", "list", "--porcelain"])
        .output()
        .expect("list worktrees");
    assert!(output.status.success());
    String::from_utf8(output.stdout)
        .expect("utf8 worktree list")
        .lines()
        .filter_map(|line| line.strip_prefix("worktree "))
        .filter_map(|path| Path::new(path).canonicalize().ok())
        .any(|path| path == member)
}
