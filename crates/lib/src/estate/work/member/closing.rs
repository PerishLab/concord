use super::Cleanup;
use crate::{Error, git};
use std::path::PathBuf;

struct Fixture {
    temp: tempfile::TempDir,
    source: PathBuf,
    path: PathBuf,
    head: String,
}

impl Fixture {
    fn new() -> Self {
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
        let head = repo.head().expect("head");
        repo.add(&path, "task/1", false).expect("Member");
        Self {
            temp,
            source,
            path,
            head,
        }
    }

    fn cleanup(&self) -> Cleanup<'_> {
        Cleanup {
            source: &self.source,
            path: &self.path,
            branch: "task/1",
            head: &self.head,
        }
    }
}

#[tokio::test(flavor = "current_thread")]
async fn blocked() {
    let fixture = Fixture::new();
    let lock = fixture.source.join(".git/refs/heads/task/1.lock");
    std::fs::write(&lock, "foreign lock").expect("refuse deletion");
    let error = fixture
        .cleanup()
        .complete(async { panic!("no settlement after failed deletion") })
        .await
        .expect_err("Git failure");
    assert_eq!(error.code(), "concord.member.cleanup");
    assert!(error.message().contains("Member restoration refused"));
    assert_eq!(
        git::at(&fixture.source)
            .text(&["rev-parse", "task/1"])
            .expect("retained ref"),
        fixture.head
    );
    std::fs::remove_file(lock).expect("release injected lock");
    fixture
        .cleanup()
        .restore()
        .expect("recovery after Git unlocks");
    assert_eq!(
        git::at(&fixture.path).head().expect("restored"),
        fixture.head
    );
    assert!(
        git::at(&fixture.source)
            .registered(&fixture.path)
            .expect("registration")
    );
}

#[tokio::test(flavor = "current_thread")]
async fn changed() {
    let fixture = Fixture::new();
    std::fs::write(fixture.path.join("README.md"), "new payload").expect("change");
    let member = git::at(&fixture.path);
    member.run(&["add", "README.md"]).expect("stage");
    member
        .run(&["commit", "-m", "external change"])
        .expect("commit");
    let changed = member.head().expect("changed head");
    let error = fixture
        .cleanup()
        .complete(async { panic!("no settlement after moved head") })
        .await
        .expect_err("expected head refuses");
    assert_eq!(error.code(), "concord.member.cleanup");
    assert!(error.message().contains("Member restoration refused"));
    assert_eq!(
        git::at(&fixture.source)
            .text(&["rev-parse", "task/1"])
            .expect("preserved ref"),
        changed
    );
    assert!(!fixture.path.exists());
}

#[tokio::test(flavor = "current_thread")]
async fn occupied() {
    for changed in [false, true] {
        let fixture = Fixture::new();
        let source = git::at(&fixture.source);
        let replacement = if changed {
            std::fs::write(fixture.source.join("README.md"), "foreign").expect("foreign payload");
            source.run(&["add", "README.md"]).expect("stage");
            source.run(&["commit", "-m", "foreign"]).expect("commit");
            source.head().expect("foreign head")
        } else {
            fixture.head.clone()
        };
        let error = fixture
            .cleanup()
            .complete(async {
                assert!(!fixture.path.exists());
                assert!(
                    !source
                        .exists("task/1")
                        .expect("branch retired before settlement")
                );
                source
                    .run(&["update-ref", "refs/heads/task/1", &replacement, ""])
                    .expect("external ref creation");
                Err(Error::new("injected settlement failure"))
            })
            .await
            .expect_err("occupied rollback");
        assert_eq!(error.code(), "concord.member.cleanup");
        assert!(error.message().contains("Member restoration refused"));
        assert_eq!(
            source.text(&["rev-parse", "task/1"]).expect("preserved"),
            replacement
        );
        assert!(!fixture.path.exists());
        assert!(fixture.temp.path().exists());
    }
}
