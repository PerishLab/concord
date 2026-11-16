mod fixture;

use super::{Seat, files, journal, payload};
use fixture::{Fixture, git};
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};

#[tokio::test(flavor = "current_thread")]
async fn lifecycle() {
    let fixture = Fixture::new().await;
    let before = payload::inventory(&fixture.task.source.join(".issues")).unwrap();
    let staged = fixture.old().join("staged");
    fs::set_permissions(&staged, fs::Permissions::from_mode(0o751)).unwrap();
    let metadata = staged.metadata().unwrap();
    let payload = payload::inventory(&fixture.task.source.join(".issues")).unwrap();
    assert!(before != payload);
    let head = git(&fixture.old(), &["rev-parse", "HEAD"]);
    let branch = git(&fixture.old(), &["symbolic-ref", "HEAD"]);
    fixture.run("stage").await.unwrap();
    assert!(!fixture.task.source.join(".concord/sudo").exists());
    assert!(
        Seat::new(journal::stage(&fixture.task.home))
            .open()
            .await
            .is_err()
    );
    fixture.run("activate").await.unwrap();
    fixture.run("activate").await.unwrap();
    assert!(fixture.run("abort").await.is_err());
    payload
        .check(&fixture.task.home.join("workspace/issues"))
        .unwrap();
    assert_eq!(git(&fixture.target(), &["show", ":staged"]), "index");
    assert_eq!(git(&fixture.target(), &["rev-parse", "HEAD"]), head);
    assert_eq!(git(&fixture.target(), &["symbolic-ref", "HEAD"]), branch);
    let actual = fixture.target().join("staged").metadata().unwrap();
    assert_eq!(
        (metadata.ino(), metadata.mtime(), metadata.mtime_nsec()),
        (actual.ino(), actual.mtime(), actual.mtime_nsec())
    );
    assert_eq!(
        fs::read_to_string(fixture.target().join("staged")).unwrap(),
        "dirty"
    );
    let estate = Seat::new(&fixture.task.home).open().await.unwrap();
    let audit = estate.settled().await.unwrap();
    assert!(audit.agrees(), "{audit:?}");
    assert_eq!(estate.issues().await.unwrap()[0].revision, 1);
    drop(estate);
    fixture.run("finish").await.unwrap();
    fixture.run("finish").await.unwrap();
    assert!(!fixture.task.source.join(".concord").exists());
    assert!(
        !journal::root(&fixture.task.home)
            .join("intent.json")
            .exists()
    );
    assert!(fixture.temp.path().exists());
}

#[tokio::test(flavor = "current_thread")]
async fn abort() {
    let fixture = Fixture::new().await;
    let before = files::inventory(&fixture.task.source.join(".concord")).unwrap();
    let payload = payload::inventory(&fixture.task.source.join(".issues")).unwrap();
    let pointer = git(&fixture.old(), &["rev-parse", "--absolute-git-dir"]);
    let index = files::digest(&std::path::Path::new(&pointer).join("index")).unwrap();
    fixture.run("stage").await.unwrap();
    fixture.run("abort").await.unwrap();
    assert_eq!(
        before,
        files::inventory(&fixture.task.source.join(".concord")).unwrap()
    );
    payload.check(&fixture.task.source.join(".issues")).unwrap();
    assert_eq!(
        index,
        files::digest(&std::path::Path::new(&pointer).join("index")).unwrap()
    );
    assert!(!journal::root(&fixture.task.home).exists());
}

#[tokio::test(flavor = "current_thread")]
async fn interruption() {
    for boundary in 0..4 {
        let fixture = Fixture::new().await;
        fixture.run("stage").await.unwrap();
        let staged = journal::stage(&fixture.task.home);
        let workspace = fixture.task.home.join("workspace");
        fs::rename(staged.join("workspace"), &workspace).unwrap();
        if boundary > 0 {
            fs::rename(
                fixture.task.source.join(".issues"),
                workspace.join("issues"),
            )
            .unwrap();
        }
        if boundary > 1 {
            let ready = super::Ready::read(&fixture.task.home).unwrap();
            ready.members[0]
                .repair(&fixture.task.source, &workspace.join("issues"))
                .await
                .unwrap();
        }
        if boundary > 2 {
            fs::rename(
                journal::possession(&fixture.task.home),
                workspace.join("sudo"),
            )
            .unwrap();
        }
        fixture.run("activate").await.unwrap();
        fs::remove_file(fixture.task.source.join(".concord/estate.sqlite3")).unwrap();
        fixture.run("finish").await.unwrap();
        fixture.run("finish").await.unwrap();
        assert_eq!(git(&fixture.target(), &["show", ":staged"]), "index");
    }
}

#[tokio::test(flavor = "current_thread")]
async fn changed() {
    let fixture = Fixture::new().await;
    fixture.run("stage").await.unwrap();
    fs::write(fixture.old().join("untracked"), "changed").unwrap();
    assert!(fixture.run("activate").await.is_err());
    assert!(fixture.run("abort").await.is_err());
    assert!(!fixture.task.home.join("workspace").exists());
    fs::write(fixture.old().join("untracked"), "untracked").unwrap();
    fixture.run("abort").await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn unknown() {
    let fixture = Fixture::new().await;
    fixture.run("stage").await.unwrap();
    let unknown = journal::stage(&fixture.task.home).join("workspace/unknown");
    fs::write(&unknown, "preserve").unwrap();
    assert!(fixture.run("activate").await.is_err());
    assert!(fixture.run("abort").await.is_err());
    assert_eq!(fs::read_to_string(&unknown).unwrap(), "preserve");
    fs::remove_file(unknown).unwrap();
    fixture.run("abort").await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn links() {
    for target in ["../../../escape", "staged/../source", "/tmp/../escape"] {
        let fixture = Fixture::new().await;
        fs::remove_file(fixture.old().join("link")).unwrap();
        std::os::unix::fs::symlink(target, fixture.old().join("link")).unwrap();
        assert!(fixture.run("stage").await.is_err());
        fixture.run("abort").await.unwrap();
        assert_eq!(
            fs::read_link(fixture.old().join("link")).unwrap(),
            std::path::Path::new(target)
        );
    }
}

#[tokio::test(flavor = "current_thread")]
async fn locked() {
    let fixture = Fixture::new().await;
    let held = journal::hold(&fixture.task.source).unwrap();
    assert!(fixture.run("stage").await.is_err());
    assert!(fixture.task.source.join(".concord/sudo").exists());
    journal::release(held, Ok(())).unwrap();
    fixture.run("stage").await.unwrap();
    let ready = super::Ready::read(&fixture.task.home).unwrap();
    let path = std::path::PathBuf::from(git(&fixture.old(), &["rev-parse", "--absolute-git-dir"]))
        .join("index.lock");
    fs::write(&path, "writer").unwrap();
    assert!(fixture.run("activate").await.is_err());
    assert_eq!(ready.members.len(), 1);
    fs::remove_file(path).unwrap();
    fixture.run("abort").await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn backlink() {
    let fixture = Fixture::new().await;
    fixture.run("stage").await.unwrap();
    let path = std::path::PathBuf::from(git(&fixture.old(), &["rev-parse", "--absolute-git-dir"]))
        .join("gitdir");
    let before = fs::read(&path).unwrap();
    fs::write(&path, "/foreign/.git\n").unwrap();
    assert!(fixture.run("activate").await.is_err());
    assert_eq!(fs::read_to_string(&path).unwrap(), "/foreign/.git\n");
    fs::write(path, before).unwrap();
    fixture.run("abort").await.unwrap();
    assert!(fixture.repository.exists());
}

#[tokio::test(flavor = "current_thread")]
async fn nested() {
    let fixture = Fixture::new().await;
    fs::create_dir_all(fixture.old().join("vendor/.git")).unwrap();
    assert!(fixture.run("stage").await.is_err());
    fixture.run("abort").await.unwrap();
    assert!(fixture.old().join("vendor/.git").is_dir());
}
