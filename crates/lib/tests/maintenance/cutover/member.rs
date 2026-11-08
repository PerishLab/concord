use super::{Seat, tests::Fixture};
use concord_core::Coordinate;

#[tokio::test(flavor = "current_thread")]
async fn member() {
    use concord_core::{Register, Repository, Start};
    let fixture = Fixture::new().await;
    let seat = Seat::new(fixture.temp.path().join("seed"));
    let estate = seat.open().await.unwrap();
    let source = fixture.temp.path().join("repository");
    let remote = fixture.temp.path().join("remote.git");
    std::fs::create_dir(&source).unwrap();
    git(&source, &["init", "-b", "main"]);
    git(&source, &["config", "user.name", "Cutover Test"]);
    git(
        &source,
        &["config", "user.email", "cutover@example.invalid"],
    );
    git(&source, &["commit", "--allow-empty", "-m", "source"]);
    git(&source, &["init", "--bare", remote.to_str().unwrap()]);
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
    estate
        .register(&Register {
            node: "R_concord".into(),
            repository: Repository::parse("PerishLab/concord").unwrap(),
            path: source,
        })
        .await
        .unwrap();
    estate
        .start(&Start {
            issue: Coordinate::parse("PerishLab/concord#452").unwrap(),
            node: "I_retained".into(),
            stable: "R_concord".into(),
            kind: "Task".into(),
            claims: vec!["crates".into()],
            revision: 0,
        })
        .await
        .unwrap();
    drop(estate);
    let worktree = fixture
        .temp
        .path()
        .join("seed/workspace/issues/I_retained/worktree");
    std::fs::write(worktree.join("staged"), "index").unwrap();
    std::fs::write(worktree.join(".gitignore"), "ignored\n").unwrap();
    git(&worktree, &["add", "staged", ".gitignore"]);
    std::fs::write(worktree.join("staged"), "dirty").unwrap();
    std::fs::write(worktree.join("untracked"), "untracked").unwrap();
    std::fs::write(worktree.join("ignored"), "ignored").unwrap();
    std::fs::copy(
        seat.database(),
        fixture.source.join(".concord/estate.sqlite3"),
    )
    .unwrap();
    assert!(fixture.run("stage").await.is_err());
    fixture.run("abort").await.unwrap();
    assert!(
        fixture
            .temp
            .path()
            .join("seed/workspace/issues/I_retained/worktree")
            .is_dir()
    );
    assert!(fixture.source.join(".concord/sudo").is_file());
    for (name, expected) in [
        ("staged", "dirty"),
        ("untracked", "untracked"),
        ("ignored", "ignored"),
    ] {
        assert_eq!(
            std::fs::read_to_string(worktree.join(name)).unwrap(),
            expected
        );
    }
    let index = std::process::Command::new("git")
        .arg("-C")
        .arg(&worktree)
        .args(["show", ":staged"])
        .output()
        .unwrap();
    assert!(index.status.success());
    assert_eq!(index.stdout, b"index");
}

fn git(root: &std::path::Path, args: &[&str]) {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
