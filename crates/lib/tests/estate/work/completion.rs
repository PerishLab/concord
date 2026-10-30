use super::start::{World, admit, world};
use concord_core::{Finish, IssueDeclaration, IssueProving, Seat, Start};
use keel::adapt::db::Sqlite;
use keel::wire::Wire;
use std::path::PathBuf;
use std::process::Command;

async fn member(world: &World) -> (Finish, PathBuf) {
    let issue = admit(world, 1, "I_complete").await;
    world
        .estate
        .start(&Start {
            issue: issue.clone(),
            node: "I_complete".into(),
            stable: "R_concord".into(),
            kind: "Task".into(),
            claims: vec!["README.md".into()],
            revision: 0,
        })
        .await
        .expect("Member");
    (
        Finish { issue, revision: 1 },
        world.temp.path().join(".issues/I_complete/worktree"),
    )
}

#[tokio::test(flavor = "current_thread")]
async fn protected() {
    for mode in ["dirty", "untracked", "commit", "proof", "pull"] {
        let world = world().await;
        let (mut request, path) = member(&world).await;
        let before = world
            .estate
            .issue_member(&request.issue)
            .await
            .expect("Member");
        let expected = protect(&world, &mut request, &path, mode).await;
        assert_eq!(
            world
                .estate
                .finish(&request)
                .await
                .expect_err("protected")
                .code(),
            expected,
            "{mode}"
        );
        let held = world
            .estate
            .issue_member(&request.issue)
            .await
            .expect("retained Member");
        assert_eq!(held.key, before.key);
        assert_eq!(held.branch, before.branch);
        assert_eq!(
            world
                .estate
                .issue(&request.issue)
                .await
                .expect("Anchor")
                .revision,
            request.revision
        );
        assert!(path.join(".git").is_file());
        assert!(world.estate.inspect().await.expect("agreement").agrees());
        if matches!(mode, "dirty" | "commit") {
            assert_eq!(
                std::fs::read_to_string(path.join("README.md")).expect("payload"),
                "preserved"
            );
        }
        if mode == "untracked" {
            assert_eq!(
                std::fs::read_to_string(path.join("valuable")).expect("payload"),
                "preserved"
            );
        }
    }
}

#[tokio::test(flavor = "current_thread")]
async fn database() {
    let world = world().await;
    let (request, path) = member(&world).await;
    let before = world
        .estate
        .issue_member(&request.issue)
        .await
        .expect("original Member");
    let mut wire = Sqlite::file(Seat::new(world.temp.path()).database())
        .await
        .expect("wire");
    wire.script("CREATE TRIGGER refuse_complete BEFORE UPDATE ON Anchor BEGIN SELECT RAISE(ABORT, 'injected completion failure'); END").await.expect("inject failure");
    let error = world
        .estate
        .finish(&request)
        .await
        .expect_err("failed batch");
    assert_eq!(error.code(), "concord.member.cleanup");
    assert!(error.message().contains("original Member restored"));
    assert_eq!(
        world
            .estate
            .issue_member(&request.issue)
            .await
            .expect("retained"),
        before
    );
    assert_eq!(
        world
            .estate
            .issue(&request.issue)
            .await
            .expect("Anchor")
            .revision,
        1
    );
    assert!(path.join(".git").is_file());
    assert!(world.estate.inspect().await.expect("agreement").agrees());
    wire.script("DROP TRIGGER refuse_complete")
        .await
        .expect("allow retry");
    drop(wire);
    assert_eq!(
        world
            .estate
            .finish(&request)
            .await
            .expect("retry same revision"),
        2
    );
    assert!(!path.exists());
    assert!(world.estate.inspect().await.expect("settled").agrees());
}

#[tokio::test(flavor = "current_thread")]
async fn foreign() {
    let world = world().await;
    let issue = admit(&world, 1, "I_foreign").await;
    git(&world.source, &["branch", "task/1"]);
    let error = world
        .estate
        .start(&Start {
            issue: issue.clone(),
            node: "I_foreign".into(),
            stable: "R_concord".into(),
            kind: "Task".into(),
            claims: vec!["README.md".into()],
            revision: 0,
        })
        .await
        .expect_err("foreign branch");
    assert_eq!(error.code(), "concord.member.pending_disagreement");
    git(
        &world.source,
        &["show-ref", "--verify", "refs/heads/task/1"],
    );
    assert_eq!(
        world
            .estate
            .issue(&issue)
            .await
            .expect("unchanged Anchor")
            .revision,
        0
    );
    assert!(
        !world
            .temp
            .path()
            .join(".issues/I_foreign/worktree")
            .exists()
    );
}

fn git(root: &std::path::Path, args: &[&str]) {
    assert!(
        Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .status()
            .expect("Git")
            .success()
    );
}

async fn protect(
    world: &World,
    request: &mut Finish,
    path: &std::path::Path,
    mode: &str,
) -> &'static str {
    match mode {
        "proof" => {
            world
                .estate
                .prove_issue(&IssueProving {
                    issue: request.issue.clone(),
                    revision: 1,
                })
                .await
                .expect("proof");
            request.revision = 2;
            "concord.member.delivery_started"
        }
        "pull" => {
            world
                .estate
                .refer_issue(&IssueDeclaration {
                    issue: request.issue.clone(),
                    provider: "github".into(),
                    owner: "PerishLab".into(),
                    repository: "concord".into(),
                    number: 2,
                    revision: 1,
                })
                .await
                .expect("pull reference");
            request.revision = 2;
            "concord.member.delivery_started"
        }
        "untracked" => {
            std::fs::write(path.join("valuable"), "preserved").expect("untracked");
            "concord.member.dirty"
        }
        _ => {
            std::fs::write(path.join("README.md"), "preserved").expect("tracked change");
            if mode == "commit" {
                git(path, &["add", "README.md"]);
                git(path, &["commit", "-m", "payload"]);
                "concord.member.changed"
            } else {
                "concord.member.dirty"
            }
        }
    }
}
