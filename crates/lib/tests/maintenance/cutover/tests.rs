use super::{Seat, entry, files, journal};
use concord_core::{Admission, Coordinate};
use std::path::PathBuf;

pub(super) struct Fixture {
    pub(super) temp: tempfile::TempDir,
    pub(super) source: PathBuf,
    pub(super) home: PathBuf,
}

impl Fixture {
    pub(super) async fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("legacy");
        let home = temp.path().join("home");
        std::fs::create_dir(&source).unwrap();
        std::fs::create_dir(&home).unwrap();
        let seat = Seat::new(temp.path().join("seed"));
        let estate = seat.bootstrap().await.unwrap();
        estate
            .admit(&Admission {
                node: "I_retained".into(),
                coordinate: Coordinate::parse("PerishLab/concord#452").unwrap(),
            })
            .await
            .unwrap();
        drop(estate);
        std::fs::create_dir(source.join(".concord")).unwrap();
        std::fs::copy(seat.database(), source.join(".concord/estate.sqlite3")).unwrap();
        std::fs::copy(seat.sudo(), source.join(".concord/sudo")).unwrap();
        let artifact = source.join(".issues/I_retained/artifacts/evidence");
        std::fs::create_dir_all(&artifact).unwrap();
        std::fs::write(artifact.join("source"), "valuable payload").unwrap();
        Self { temp, source, home }
    }

    pub(super) async fn run(&self, command: &str) -> concord_core::Result<()> {
        entry(vec![
            command.into(),
            self.source.clone().into(),
            self.home.clone().into(),
            "--stopped".into(),
        ])
        .await
    }
}

#[tokio::test(flavor = "current_thread")]
async fn lifecycle() {
    let fixture = Fixture::new().await;
    let before = files::inventory(&fixture.source.join(".concord")).unwrap();
    fixture.run("stage").await.unwrap();
    assert!(!fixture.source.join(".concord/sudo").exists());
    assert!(!fixture.home.join("workspace").exists());
    let stage = Seat::new(journal::stage(&fixture.home))
        .open()
        .await
        .unwrap();
    let identity = stage.identity().to_string();
    assert_eq!(stage.issues().await.unwrap()[0].node, "I_retained");
    drop(stage);
    fixture.run("activate").await.unwrap();
    fixture.run("activate").await.unwrap();
    assert!(fixture.run("abort").await.is_err());
    let estate = Seat::new(&fixture.home).open().await.unwrap();
    assert_eq!(estate.identity(), identity);
    assert_eq!(estate.issues().await.unwrap()[0].revision, 0);
    drop(estate);
    assert_eq!(
        files::digest(&fixture.home.join("workspace/estate.sqlite3")).unwrap(),
        before.files[std::path::Path::new("estate.sqlite3")]
    );
    fixture.run("finish").await.unwrap();
    fixture.run("finish").await.unwrap();
    assert!(!fixture.source.join(".concord").exists());
    assert!(!fixture.source.join(".issues").exists());
    assert_eq!(
        std::fs::read_to_string(
            fixture
                .home
                .join("workspace/issues/I_retained/artifacts/evidence/source")
        )
        .unwrap(),
        "valuable payload"
    );
    assert!(journal::root(&fixture.home).join("complete.json").is_file());
    assert!(fixture.temp.path().exists());
}

#[tokio::test(flavor = "current_thread")]
async fn rollback() {
    let fixture = Fixture::new().await;
    let before = files::inventory(&fixture.source.join(".concord")).unwrap();
    fixture.run("stage").await.unwrap();
    assert!(fixture.run("stage").await.is_err());
    fixture.run("abort").await.unwrap();
    assert_eq!(
        files::inventory(&fixture.source.join(".concord")).unwrap(),
        before
    );
    assert!(!journal::root(&fixture.home).exists());
    assert!(!fixture.home.join("workspace").exists());
}

#[tokio::test(flavor = "current_thread")]
async fn refusals() {
    let fixture = Fixture::new().await;
    let sidecar = fixture.source.join(".concord/estate.sqlite3-wal");
    std::fs::write(&sidecar, "unknown").unwrap();
    assert!(fixture.run("stage").await.is_err());
    assert!(fixture.source.join(".concord/sudo").exists());
    std::fs::remove_file(sidecar).unwrap();
    std::fs::create_dir(fixture.home.join("workspace")).unwrap();
    assert!(fixture.run("stage").await.is_err());
    std::fs::remove_dir(fixture.home.join("workspace")).unwrap();
    fixture.run("stage").await.unwrap();
    let artifact = fixture
        .source
        .join(".issues/I_retained/artifacts/evidence/source");
    std::fs::write(&artifact, "changed valuable payload").unwrap();
    assert!(fixture.run("activate").await.is_err());
    assert!(!fixture.home.join("workspace").exists());
    assert!(!fixture.source.join(".concord/sudo").exists());
    std::fs::write(&artifact, "valuable payload").unwrap();
    fixture.run("abort").await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn incompatible() {
    use keel::{adapt::db::Sqlite, wire::Wire};
    let fixture = Fixture::new().await;
    let mut wire = Sqlite::file(fixture.source.join(".concord/estate.sqlite3"))
        .await
        .unwrap();
    wire.script("CREATE TABLE Task(id INTEGER PRIMARY KEY NOT NULL)")
        .await
        .unwrap();
    drop(wire);
    let before = files::inventory(&fixture.source.join(".concord")).unwrap();
    assert!(fixture.run("stage").await.is_err());
    assert!(!fixture.home.join("workspace").exists());
    fixture.run("abort").await.unwrap();
    assert_eq!(
        files::inventory(&fixture.source.join(".concord")).unwrap(),
        before
    );
}

#[tokio::test(flavor = "current_thread")]
async fn interrupted() {
    let fixture = Fixture::new().await;
    std::fs::create_dir_all(journal::root(&fixture.home)).unwrap();
    std::fs::write(
        journal::root(&fixture.home).join("intent.pending"),
        "incomplete",
    )
    .unwrap();
    fixture.run("abort").await.unwrap();
    fixture.run("stage").await.unwrap();
    let staged = journal::stage(&fixture.home);
    let inventory = files::inventory(&staged).unwrap();
    files::erase(&staged, &inventory).unwrap();
    std::fs::remove_file(journal::root(&fixture.home).join("ready.json")).unwrap();
    fixture.run("abort").await.unwrap();
    fixture.run("stage").await.unwrap();
    fixture.run("activate").await.unwrap();
    std::fs::remove_file(fixture.source.join(".concord/estate.sqlite3")).unwrap();
    std::fs::write(
        journal::root(&fixture.home).join("complete.pending"),
        "interrupted receipt",
    )
    .unwrap();
    fixture.run("finish").await.unwrap();
    assert!(!fixture.source.join(".concord").exists());
    assert!(
        fixture
            .home
            .join("workspace/issues/I_retained/artifacts/evidence/source")
            .is_file()
    );
}

#[tokio::test(flavor = "current_thread")]
async fn occupied() {
    let fixture = Fixture::new().await;
    let intent = super::Cutover {
        source: fixture.source.clone(),
        home: fixture.home.clone(),
    };
    let guards = intent.guards().unwrap();
    assert!(fixture.run("stage").await.is_err());
    assert!(fixture.source.join(".concord/sudo").exists());
    drop(guards);
    fixture.run("stage").await.unwrap();
    let staged = journal::stage(&fixture.home).join("workspace");
    std::fs::write(staged.join("unknown"), "preserve").unwrap();
    assert!(fixture.run("activate").await.is_err());
    assert!(fixture.run("abort").await.is_err());
    assert_eq!(
        std::fs::read_to_string(staged.join("unknown")).unwrap(),
        "preserve"
    );
    std::fs::remove_file(staged.join("unknown")).unwrap();
    fixture.run("abort").await.unwrap();
}

#[cfg(unix)]
#[tokio::test(flavor = "current_thread")]
async fn links() {
    let fixture = Fixture::new().await;
    let source = fixture
        .source
        .join(".issues/I_retained/artifacts/evidence/source");
    let link = source.with_file_name("link");
    std::os::unix::fs::symlink(&source, &link).unwrap();
    assert!(fixture.run("stage").await.is_err());
    assert!(fixture.source.join(".concord/sudo").is_file());
    assert!(!journal::root(&fixture.home).exists());
}

#[cfg(windows)]
#[tokio::test(flavor = "current_thread")]
async fn links() {
    let fixture = Fixture::new().await;
    let target = fixture.temp.path().join("outside");
    std::fs::create_dir(&target).unwrap();
    let link = fixture.source.join(".issues/I_retained/artifacts/junction");
    let output = std::process::Command::new("cmd.exe")
        .args(["/d", "/c", "mklink", "/J"])
        .arg(link.to_string_lossy().replace('/', "\\"))
        .arg(target.to_string_lossy().replace('/', "\\"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(fixture.run("stage").await.is_err());
    assert!(fixture.source.join(".concord/sudo").is_file());
    std::fs::remove_dir(link).unwrap();
    assert!(target.is_dir());
}
