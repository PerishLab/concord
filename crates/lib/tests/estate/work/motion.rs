use concord_core::{Attach, Rehome, Rename, Seat};
use std::{path::Path, process::Command};

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
