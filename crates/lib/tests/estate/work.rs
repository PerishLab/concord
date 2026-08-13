use concord_core::{
    Attach, BoundaryState, Claiming, Finish, IntegrationState, Life, Proving, Rehome, Release,
    Rename, Seat,
};
use std::{path::Path, process::Command};

#[tokio::test(flavor = "current_thread")]
async fn member() {
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
            remote.to_str().expect("remote path"),
        ],
    );
    git(&source, &["push", "-u", "origin", "main"]);

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
    let path = temp.path().join("local/.tasks/work/repo");
    assert!(path.join(".git").is_file());
    let status = estate
        .member_status("work", "repo")
        .await
        .expect("read Member status");
    assert_eq!(status.boundary, BoundaryState::Absent);
    assert_eq!(status.integration, IntegrationState::Reachable);
    assert!(status.worktree.clean);
    assert!(status.integration_checkout.clean);
    assert!(status.local_upstream.is_none());
    git(&path, &["push", "-u", "origin", "work"]);
    let status = estate
        .member_status("local/work", "repo")
        .await
        .expect("read tracked Member status");
    let upstream = status.local_upstream.expect("local upstream");
    assert_eq!(
        (upstream.reference.as_str(), upstream.ahead, upstream.behind),
        ("origin/work", 0, 0)
    );
    assert!(
        status
            .local_tracking_refs
            .iter()
            .any(|reference| reference == "origin/work")
    );

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
    assert_eq!(
        estate
            .member_status("local/work", "repo")
            .await
            .expect("current Boundary status")
            .boundary,
        BoundaryState::Current
    );
    std::fs::create_dir(path.join("docs")).expect("docs directory");
    std::fs::write(path.join("docs/note.md"), "member delta\n").expect("member delta");
    git(&path, &["add", "docs/note.md"]);
    git(&path, &["commit", "-m", "member delta"]);
    let status = estate
        .member_status("local/work", "repo")
        .await
        .expect("stale Boundary status");
    assert_eq!(status.boundary, BoundaryState::Stale);
    assert_eq!(status.integration, IntegrationState::Unlanded);
    assert_eq!(status.local_upstream.expect("upstream").ahead, 1);
    assert!(status.local_tracking_refs.is_empty());
    std::fs::write(path.join("README.md"), "dirty\n").expect("tracked change");
    std::fs::write(path.join("scratch.txt"), "untracked\n").expect("untracked file");
    let status = estate
        .member_status("local/work", "repo")
        .await
        .expect("dirty Member status");
    assert_eq!(
        (
            status.worktree.tracked_changes,
            status.worktree.untracked_files
        ),
        (1, 1)
    );
    assert!(!status.worktree.clean);
    git(&path, &["checkout", "--", "README.md"]);
    std::fs::remove_file(path.join("scratch.txt")).expect("remove fixture scratch");

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
    git(&path, &["push"]);
    git(&source, &["cherry-pick", "--no-commit", "work"]);
    git(&source, &["commit", "-m", "equivalent member delta"]);
    assert_eq!(
        estate
            .member_status("local/work", "repo")
            .await
            .expect("tree-equivalent status")
            .integration,
        IntegrationState::TreeEquivalent
    );
    git(&source, &["merge", "--no-ff", "work", "-m", "land member"]);
    assert_eq!(
        estate
            .member_status("local/work", "repo")
            .await
            .expect("landed status")
            .integration,
        IntegrationState::Reachable
    );
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
