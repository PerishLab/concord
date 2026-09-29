use concord_core::{Admission, Coordinate, Finish, Register, Repository, Seat, Start};
use std::path::{Path, PathBuf};
use std::process::Command;

struct World {
    estate: concord_core::Estate,
    source: PathBuf,
    remote: PathBuf,
    temp: tempfile::TempDir,
}

#[tokio::test(flavor = "current_thread")]
async fn provisioning() {
    let world = world().await;
    let issue = admit(&world, 26, "I_execution").await;
    let upstream = world.temp.path().join("upstream");
    git(
        world.temp.path(),
        &[
            "clone",
            world.remote.to_str().expect("remote"),
            upstream.to_str().expect("upstream"),
        ],
    );
    identity(&upstream);
    git(&upstream, &["checkout", "main"]);
    std::fs::write(upstream.join("UPSTREAM.md"), "advanced\n").expect("upstream delta");
    git(&upstream, &["add", "UPSTREAM.md"]);
    git(&upstream, &["commit", "-m", "advance main"]);
    git(&upstream, &["push", "origin", "main"]);
    let advanced = text(&upstream, &["rev-parse", "HEAD"]);
    let request = Start {
        issue: issue.clone(),
        node: "I_execution".to_string(),
        stable: "R_concord".to_string(),
        claims: vec!["crates".to_string()],
        revision: 0,
    };
    let member = world
        .estate
        .start(&request)
        .await
        .expect("start Member")
        .member;
    assert_eq!(member.base, advanced);
    assert_eq!(text(&world.source, &["rev-parse", "HEAD"]), advanced);
    assert_eq!(
        world
            .estate
            .start(&request)
            .await
            .expect("replay exact start")
            .member,
        member
    );
    assert_eq!(world.estate.issue(&issue).await.expect("Issue").revision, 1);
    let path = world.temp.path().join(".issues/I_execution/worktree");
    std::fs::create_dir(path.join("crates")).expect("claim directory");
    std::fs::write(path.join("crates/note.md"), "changed\n").expect("member change");
    git(&path, &["add", "crates/note.md"]);
    git(&path, &["commit", "-m", "change"]);
    let refused = world
        .estate
        .finish(&Finish { issue, revision: 1 })
        .await
        .expect_err("changed Member cannot finish unchanged");
    assert_eq!(refused.code(), "concord.member.changed");
}

#[tokio::test(flavor = "current_thread")]
async fn unchanged() {
    let world = world().await;
    let issue = admit(&world, 27, "I_research").await;
    world
        .estate
        .start(&Start {
            issue: issue.clone(),
            node: "I_research".to_string(),
            stable: "R_concord".to_string(),
            claims: vec!["docs".to_string()],
            revision: 0,
        })
        .await
        .expect("start research");
    assert_eq!(
        world
            .estate
            .finish(&Finish {
                issue: issue.clone(),
                revision: 1,
            })
            .await
            .expect("finish unchanged research"),
        2
    );
    assert_eq!(
        world
            .estate
            .issue_member(&issue)
            .await
            .expect_err("Member removed")
            .code(),
        "concord.member.absent"
    );
}

async fn world() -> World {
    let temp = tempfile::tempdir().expect("temporary Space");
    let source = temp.path().join("source");
    let remote = temp.path().join("remote.git");
    std::fs::create_dir(&source).expect("source");
    std::fs::create_dir(&remote).expect("remote");
    git(&remote, &["init", "--bare"]);
    git(&source, &["init", "-b", "main"]);
    identity(&source);
    std::fs::write(source.join("README.md"), "fixture\n").expect("fixture");
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
    let estate = Seat::new(temp.path())
        .bootstrap()
        .await
        .expect("bootstrap estate");
    estate
        .register(&Register {
            node: "R_concord".to_string(),
            repository: Repository::parse("PerishLab/concord").expect("repository"),
            path: source.clone(),
        })
        .await
        .expect("register Integration");
    World {
        estate,
        source,
        remote,
        temp,
    }
}

async fn admit(world: &World, number: i64, node: &str) -> Coordinate {
    let issue =
        Coordinate::parse(&format!("PerishLab/concord#{number}")).expect("Issue coordinate");
    world
        .estate
        .admit(&Admission {
            node: node.to_string(),
            coordinate: issue.clone(),
        })
        .await
        .expect("admit Issue");
    issue
}

fn identity(root: &Path) {
    git(root, &["config", "user.name", "Concord Test"]);
    git(root, &["config", "user.email", "concord@example.invalid"]);
}

fn git(root: &Path, arguments: &[&str]) {
    assert!(
        Command::new("git")
            .arg("-C")
            .arg(root)
            .args(arguments)
            .status()
            .expect("run Git")
            .success()
    );
}

fn text(root: &Path, arguments: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(arguments)
        .output()
        .expect("run Git");
    assert!(output.status.success());
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}
