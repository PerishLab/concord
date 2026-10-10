use super::Cleanup;
use crate::{Admission, Coordinate, Seat, git};
use keel::adapt::db::Sqlite;
use keel::wire::Wire;

#[tokio::test(flavor = "current_thread")]
async fn unchanged() {
    failure(false).await;
}

#[tokio::test(flavor = "current_thread")]
async fn landed() {
    failure(true).await;
}

async fn failure(changed: bool) {
    let temp = tempfile::tempdir().expect("fixture");
    let source = temp.path().join("source");
    let path = temp.path().join("member");
    std::fs::create_dir(&source).expect("source");
    let repo = git::at(&source);
    repo.run(&["init", "-b", "main"]).expect("init");
    repo.run(&["config", "user.name", "Concord Test"])
        .expect("name");
    repo.run(&["config", "user.email", "concord@example.invalid"])
        .expect("email");
    std::fs::write(source.join("README.md"), "base").expect("base");
    repo.run(&["add", "README.md"]).expect("stage");
    repo.run(&["commit", "-m", "base"]).expect("commit");
    repo.add(&path, "task/1", false).expect("Member");
    if changed {
        std::fs::write(path.join("README.md"), "landed").expect("change");
        git::at(&path).run(&["add", "README.md"]).expect("stage");
        git::at(&path)
            .run(&["commit", "-m", "landed"])
            .expect("commit");
        repo.run(&["merge", "--ff-only", "task/1"]).expect("landed");
    }
    let head = git::at(&path).head().expect("head");
    let seat = Seat::new(temp.path());
    let estate = seat.bootstrap().await.expect("estate");
    let issue = Coordinate::parse("PerishLab/concord#1").expect("Issue");
    estate
        .admit(&Admission {
            node: "I_cleanup".into(),
            coordinate: issue.clone(),
        })
        .await
        .expect("Anchor");
    let anchor = estate.issue(&issue).await.expect("Anchor");
    let mut wire = Sqlite::file(seat.database()).await.expect("fixture wire");
    wire.script("CREATE TRIGGER refuse_cleanup BEFORE UPDATE ON Anchor BEGIN SELECT RAISE(ABORT, 'injected cleanup database failure'); END").await.expect("database failure injection");
    drop(wire);
    let cleanup = Cleanup {
        source: &source,
        path: &path,
        branch: "task/1",
        head: &head,
    };
    let error = cleanup
        .settle(async {
            assert!(
                !path.exists(),
                "external cleanup must precede the failing batch"
            );
            estate
                .core
                .batch(async |tx| tx.set("Anchor", anchor.key, &[("revision", "1")]).await)
                .await
                .map_err(crate::estate::fault)
        })
        .await
        .expect_err("deterministic Keel batch failure");
    assert_eq!(error.code(), "concord.member.cleanup");
    assert!(
        error
            .message()
            .contains("injected cleanup database failure")
    );
    assert_eq!(
        estate
            .issue(&issue)
            .await
            .expect("retained Anchor")
            .revision,
        0
    );
    assert!(error.message().contains("original Member restored"));
    assert_eq!(git::at(&path).head().expect("restored head"), head);
    assert_eq!(git::at(&path).branch().expect("branch"), "task/1");
    assert!(repo.registered(&path).expect("registered"));
    assert!(git::at(&path).clean().expect("clean"));
    cleanup.restore().expect("repeated restoration");
}

#[cfg(windows)]
#[test]
fn occupied() {
    let temp = tempfile::tempdir().expect("fixture");
    let source = temp.path().join("source");
    let path = temp.path().join("member");
    std::fs::create_dir(&source).expect("source");
    let repo = git::at(&source);
    repo.run(&["init", "-b", "main"]).expect("init");
    repo.run(&["config", "user.name", "Concord Test"])
        .expect("name");
    repo.run(&["config", "user.email", "concord@example.invalid"])
        .expect("email");
    std::fs::write(source.join("README.md"), "base").expect("base");
    repo.run(&["add", "README.md"]).expect("stage");
    repo.run(&["commit", "-m", "base"]).expect("commit");
    repo.add(&path, "task/1", false).expect("Member");
    let head = git::at(&path).head().expect("head");
    let output = std::process::Command::new("git")
        .current_dir(&path)
        .arg("-C")
        .arg(&source)
        .args(["worktree", "remove"])
        .arg(&path)
        .output()
        .expect("occupied removal");
    assert!(
        !output.status.success(),
        "Windows must refuse deleting the child's cwd"
    );
    assert!(
        !repo
            .registered(&path)
            .expect("registration removed before directory failure")
    );
    Cleanup {
        source: &source,
        path: &path,
        branch: "task/1",
        head: &head,
    }
    .restore()
    .expect("native recovery of partial Windows removal");
    assert_eq!(git::at(&path).head().expect("head"), head);
    assert!(repo.registered(&path).expect("restored registration"));
    assert!(git::at(&path).clean().expect("restored clean source"));
}
