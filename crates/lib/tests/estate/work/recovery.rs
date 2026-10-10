use super::start::{World, admit, world};
use concord_core::{Finish, Recovery, Start};
use std::path::Path;
use std::process::Command;

async fn member(world: &World, number: i64, node: &str) -> Recovery {
    let issue = admit(world, number, node).await;
    let held = world
        .estate
        .start(&Start {
            issue: issue.clone(),
            node: node.into(),
            stable: "R_concord".into(),
            kind: "Task".into(),
            claims: vec!["docs".into()],
            revision: 0,
        })
        .await
        .expect("start")
        .member;
    Recovery {
        issue,
        revision: 1,
        head: held.base,
    }
}

#[tokio::test(flavor = "current_thread")]
async fn unchanged() {
    let world = world().await;
    let request = member(&world, 1, "I_recovery").await;
    let path = world.temp.path().join(".issues/I_recovery/worktree");
    git(
        &world.source,
        &["worktree", "remove", path.to_str().expect("path")],
    );
    std::fs::remove_dir(path.parent().expect("Issue seat")).expect("absent Issue seat");
    let before = world
        .estate
        .issue_member(&request.issue)
        .await
        .expect("Member");
    let error = world
        .estate
        .finish(&Finish {
            issue: request.issue.clone(),
            revision: 1,
        })
        .await
        .expect_err("normal audit still refuses");
    assert_eq!(error.code(), "concord.audit.refused");
    assert_eq!(world.estate.recover(&request).await.expect("recover"), 1);
    assert_eq!(world.estate.recover(&request).await.expect("repeat"), 1);
    assert_eq!(
        world
            .estate
            .issue_member(&request.issue)
            .await
            .expect("Member"),
        before
    );
    assert!(world.estate.inspect().await.expect("audit").agrees());
    assert_eq!(
        world
            .estate
            .finish(&Finish {
                issue: request.issue.clone(),
                revision: 1
            })
            .await
            .expect("normal completion"),
        2
    );
    assert_eq!(
        world
            .estate
            .recover(&request)
            .await
            .expect_err("stale after completion")
            .code(),
        "concord.issue.stale"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn payload() {
    let world = world().await;
    let request = member(&world, 1, "I_recovery").await;
    let path = world.temp.path().join(".issues/I_recovery/worktree");
    git(
        &world.source,
        &["worktree", "remove", path.to_str().expect("path")],
    );
    let wrong = Recovery {
        head: "0".repeat(40),
        ..request.clone()
    };
    assert_eq!(
        world
            .estate
            .recover(&wrong)
            .await
            .expect_err("wrong head")
            .code(),
        "concord.member.recovery"
    );
    let stale = Recovery {
        revision: 0,
        ..request.clone()
    };
    assert_eq!(
        world
            .estate
            .recover(&stale)
            .await
            .expect_err("stale revision")
            .code(),
        "concord.issue.stale"
    );
    std::fs::create_dir(&path).expect("residual directory");
    std::fs::write(path.join("valuable.txt"), "preserved").expect("payload");
    assert_eq!(
        world
            .estate
            .recover(&request)
            .await
            .expect_err("nonempty path")
            .code(),
        "concord.member.recovery"
    );
    assert_eq!(
        std::fs::read_to_string(path.join("valuable.txt")).expect("payload"),
        "preserved"
    );
    std::fs::remove_file(path.join("valuable.txt")).expect("fixture cleanup");
    world
        .estate
        .recover(&request)
        .await
        .expect("empty residual path");
    std::fs::write(path.join("dirty.txt"), "preserved").expect("dirty");
    assert_eq!(
        world
            .estate
            .recover(&request)
            .await
            .expect_err("dirty existing Member")
            .code(),
        "concord.member.recovery"
    );
    assert_eq!(
        std::fs::read_to_string(path.join("dirty.txt")).expect("dirty"),
        "preserved"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn branch() {
    let world = world().await;
    let request = member(&world, 1, "I_recovery").await;
    let path = world.temp.path().join(".issues/I_recovery/worktree");
    std::fs::write(path.join("README.md"), "changed").expect("changed");
    git(&path, &["add", "README.md"]);
    git(&path, &["commit", "-m", "changed"]);
    let changed = text(&path, &["rev-parse", "HEAD"]);
    git(
        &world.source,
        &["worktree", "remove", path.to_str().expect("path")],
    );
    assert_eq!(
        world
            .estate
            .recover(&request)
            .await
            .expect_err("branch changed")
            .code(),
        "concord.member.recovery"
    );
    assert_eq!(text(&world.source, &["rev-parse", "task/1"]), changed);
    assert!(!path.exists());
}

#[tokio::test(flavor = "current_thread")]
async fn unrelated() {
    let world = world().await;
    let request = member(&world, 1, "I_recovery").await;
    member(&world, 2, "I_other").await;
    let path = world.temp.path().join(".issues/I_recovery/worktree");
    let other = world.temp.path().join(".issues/I_other/worktree");
    git(
        &world.source,
        &["worktree", "remove", path.to_str().expect("path")],
    );
    git(
        &world.source,
        &["worktree", "remove", other.to_str().expect("other")],
    );
    assert_eq!(
        world
            .estate
            .recover(&request)
            .await
            .expect_err("unrelated fault")
            .code(),
        "concord.audit.refused"
    );
    assert!(!path.exists());
    assert!(!other.exists());
}

#[cfg(unix)]
#[tokio::test(flavor = "current_thread")]
async fn symlink() {
    let world = world().await;
    let request = member(&world, 1, "I_recovery").await;
    let path = world.temp.path().join(".issues/I_recovery/worktree");
    git(
        &world.source,
        &["worktree", "remove", path.to_str().expect("path")],
    );
    std::os::unix::fs::symlink(&world.source, &path).expect("symlink");
    assert_eq!(
        world
            .estate
            .recover(&request)
            .await
            .expect_err("symlink")
            .code(),
        "concord.member.recovery"
    );
    assert_eq!(text(&world.source, &["rev-parse", "HEAD"]), request.head);
}

#[tokio::test(flavor = "current_thread")]
async fn locking() {
    let world = world().await;
    let request = member(&world, 1, "I_recovery").await;
    let path = world.temp.path().join(".issues/I_recovery/worktree");
    git(
        &world.source,
        &["worktree", "remove", path.to_str().expect("path")],
    );
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(world.temp.path().join(".concord.lock"))
        .expect("lock");
    lock.lock().expect("writer");
    let held = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(500));
        lock.unlock().expect("unlock");
    });
    let began = std::time::Instant::now();
    world
        .estate
        .recover(&request)
        .await
        .expect("locked recovery");
    assert!(began.elapsed() >= std::time::Duration::from_millis(400));
    held.join().expect("writer");
}

fn git(root: &Path, args: &[&str]) {
    assert!(
        Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .status()
            .expect("git")
            .success()
    );
}

fn text(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .expect("git");
    assert!(output.status.success());
    String::from_utf8_lossy(&output.stdout).trim().into()
}
