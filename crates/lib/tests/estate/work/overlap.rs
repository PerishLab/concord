use concord_core::{
    Admission, CommittedEvidence, CommittedOverlap, Coordinate, IssueClaiming, IssueProving,
    Register, Repository, Seat, Start,
};
use std::path::{Path, PathBuf};
use std::process::Command;

struct World {
    estate: concord_core::Estate,
    temp: tempfile::TempDir,
}

#[tokio::test(flavor = "current_thread")]
async fn evidence() {
    let world = World::new().await;
    let left = world.start(41, "I_left", "R_concord").await;
    let right = world.start(42, "I_right", "R_concord").await;
    let a = world.path("I_left");
    let b = world.path("I_right");
    commit(&a, "crates/a.rs", "left");
    commit(&b, "crates/b.rs", "right");

    let observed = world.claim(&right, 1, "docs").await;
    assert_eq!(observed, CommittedOverlap::DeclaredOnly);

    commit(&a, "crates/common.rs", "left common");
    commit(&b, "crates/common.rs", "right common");
    let observed = world.claim(&right, 2, "examples").await;
    assert_eq!(
        observed,
        CommittedOverlap::Intersecting {
            paths: vec!["crates/common.rs".to_string()]
        }
    );

    std::fs::write(a.join("dirty.rs"), "dirty\n").expect("dirty path");
    let observed = world.claim(&right, 3, "tools").await;
    assert_eq!(observed, unavailable(&left, CommittedEvidence::Dirty));
    std::fs::remove_file(a.join("dirty.rs")).expect("remove dirty path");

    world
        .estate
        .prove_issue(&IssueProving {
            issue: left.clone(),
            revision: 1,
        })
        .await
        .expect("prove left Member");
    commit(&a, "crates/later.rs", "move left HEAD");
    let observed = world.claim(&right, 4, "benches").await;
    assert_eq!(observed, unavailable(&left, CommittedEvidence::Stale));

    let foreign = world.foreign().await;
    assert!(foreign.observations.is_empty());
}

impl World {
    async fn new() -> Self {
        let temp = tempfile::tempdir().expect("temporary Space");
        let source = repository(temp.path(), "concord", "PerishLab/concord");
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
        Self { estate, temp }
    }

    async fn start(&self, number: i64, node: &str, stable: &str) -> Coordinate {
        let issue =
            Coordinate::parse(&format!("PerishLab/concord#{number}")).expect("Issue coordinate");
        self.estate
            .admit(&Admission {
                node: node.to_string(),
                coordinate: issue.clone(),
            })
            .await
            .expect("admit Issue");
        self.estate
            .start(&Start {
                issue: issue.clone(),
                node: node.to_string(),
                stable: stable.to_string(),
                claims: vec!["crates".to_string()],
                revision: 0,
            })
            .await
            .expect("start Member");
        issue
    }

    async fn claim(&self, issue: &Coordinate, revision: i64, path: &str) -> CommittedOverlap {
        let changed = self
            .estate
            .claim_issue(&IssueClaiming {
                issue: issue.clone(),
                claims: vec![path.to_string()],
                revision,
            })
            .await
            .expect("expand Claim");
        let observation = changed.observations.first().expect("Claim overlap");
        assert_eq!(observation.code, "claim.overlap");
        assert_eq!(observation.paths, vec!["crates"]);
        observation.committed.clone()
    }

    async fn foreign(&self) -> concord_core::IssueMemberChange {
        let source = repository(self.temp.path(), "plumb", "PerishLab/plumb");
        self.estate
            .register(&Register {
                node: "R_plumb".to_string(),
                repository: Repository::parse("PerishLab/plumb").expect("repository"),
                path: source,
            })
            .await
            .expect("register foreign Integration");
        let issue = Coordinate::parse("PerishLab/plumb#43").expect("foreign Issue");
        self.estate
            .admit(&Admission {
                node: "I_foreign".to_string(),
                coordinate: issue.clone(),
            })
            .await
            .expect("admit foreign Issue");
        self.estate
            .start(&Start {
                issue,
                node: "I_foreign".to_string(),
                stable: "R_plumb".to_string(),
                claims: vec!["crates".to_string()],
                revision: 0,
            })
            .await
            .expect("start foreign Member")
    }

    fn path(&self, node: &str) -> PathBuf {
        self.temp.path().join(".issues").join(node).join("worktree")
    }
}

fn repository(root: &Path, name: &str, coordinate: &str) -> PathBuf {
    let source = root.join(name);
    let remote = root.join(format!("{name}.git"));
    std::fs::create_dir(&source).expect("source directory");
    std::fs::create_dir(&remote).expect("remote directory");
    git(&remote, &["init", "--bare"]);
    git(&source, &["init", "-b", "main"]);
    git(&source, &["config", "user.name", "Concord Test"]);
    git(
        &source,
        &["config", "user.email", "concord@example.invalid"],
    );
    std::fs::write(source.join("README.md"), "fixture\n").expect("fixture");
    git(&source, &["add", "README.md"]);
    git(&source, &["commit", "-m", "fixture"]);
    git(
        &source,
        &[
            "remote",
            "add",
            "origin",
            &format!("https://github.com/{coordinate}.git"),
        ],
    );
    git(
        &source,
        &[
            "config",
            &format!("url.{}.insteadOf", remote.display()),
            &format!("https://github.com/{coordinate}.git"),
        ],
    );
    git(&source, &["push", "-u", "origin", "main"]);
    source
}

fn unavailable(member: &Coordinate, reason: CommittedEvidence) -> CommittedOverlap {
    CommittedOverlap::Unavailable {
        member: member.identity(),
        reason,
    }
}

fn commit(root: &Path, path: &str, message: &str) {
    let target = root.join(path);
    std::fs::create_dir_all(target.parent().expect("path parent")).expect("path parent");
    std::fs::write(target, format!("{message}\n")).expect("changed file");
    git(root, &["add", path]);
    git(root, &["commit", "-m", message]);
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
