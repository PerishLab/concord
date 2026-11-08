mod work;

use concord_core::Seat;
use keel::adapt::db::Sqlite;
use keel::wire::Wire;

mod legacy {
    use keel::atom::{int, string};
    use keel::resource;

    #[resource]
    struct Anchor {
        #[field(string, unique)]
        node: string,
        #[field(string)]
        owner: string,
        #[field(string)]
        repository: string,
        #[field(int, min = 1)]
        number: int,
        #[field(int, min = 0)]
        revision: int,
    }

    #[resource]
    struct IssueMember {
        #[field(string, unique = anchor)]
        name: string,
        #[field(string)]
        source: string,
        #[field(string)]
        branch: string,
        #[relation(Anchor, many2one, root)]
        anchor: Anchor,
    }

    pub fn graph() -> keel::Graph {
        let mut graph = keel::Graph::new();
        graph.plug::<Anchor>().plug::<IssueMember>();
        graph
    }
}

#[tokio::test(flavor = "current_thread")]
async fn genesis() {
    let temp = tempfile::tempdir().expect("temporary Space");
    let seat = Seat::new(temp.path());
    let estate = seat.bootstrap().await.expect("bootstrap Issue estate");
    assert!(estate.inspect().await.expect("audit").agrees());
    drop(estate);
    drop(seat.open().await.expect("exact replay"));
    assert!(seat.database().is_file());
    assert!(seat.sudo().is_file());

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let database = std::fs::metadata(seat.database())
            .expect("database metadata")
            .permissions()
            .mode()
            & 0o777;
        let sudo = std::fs::metadata(seat.sudo())
            .expect("sudo metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!((database, sudo), (0o600, 0o600));
    }
}

#[tokio::test(flavor = "current_thread")]
async fn drift() {
    let temp = tempfile::tempdir().expect("temporary Space");
    let seat = Seat::new(temp.path());
    drop(seat.bootstrap().await.expect("bootstrap estate"));

    let mut wire = Sqlite::file(seat.database()).await.expect("open database");
    wire.script("CREATE TABLE Task (id INTEGER PRIMARY KEY NOT NULL)")
        .await
        .expect("inject Task-era shape");
    drop(wire);

    let error = match seat.open().await {
        Ok(_) => panic!("non-Issue graph must refuse"),
        Err(error) => error,
    };
    assert_eq!(error.code(), "concord.estate.upgrade_required");
    assert!(error.to_string().contains("estate drift"));
}

#[tokio::test(flavor = "current_thread")]
async fn legacy() {
    let temp = tempfile::tempdir().expect("temporary Space");
    let seat = Seat::new(temp.path());
    std::fs::create_dir_all(temp.path().join("workspace/locks")).expect("estate root");
    std::fs::write(
        temp.path().join("workspace/identity.json"),
        serde_json::json!({"schema":"concord.space/v1","id":"a".repeat(64)}).to_string(),
    )
    .unwrap();
    let wire = Sqlite::file(seat.database()).await.expect("open database");
    let mut held = keel::bootstrap(legacy::graph(), wire).expect("bootstrap previous graph");
    let sudo = held.mint().await.expect("mint possession");
    std::fs::write(seat.sudo(), &sudo).expect("write possession");
    drop(held.seal(&sudo).await.expect("seal previous estate"));

    let error = match seat.open().await {
        Ok(_) => panic!("previous estate must refuse"),
        Err(error) => error,
    };
    assert_eq!(error.code(), "concord.estate.upgrade_required");
}

#[tokio::test(flavor = "current_thread")]
async fn absence() {
    let temp = tempfile::tempdir().expect("temporary Space");
    let seat = Seat::new(temp.path());
    let error = match seat.open().await {
        Ok(_) => panic!("absent estate must refuse"),
        Err(error) => error,
    };
    assert_eq!(error.code(), "concord.estate.absent");
    assert!(!temp.path().join("workspace").exists());
}

#[tokio::test(flavor = "current_thread")]
async fn identity() {
    let temp = tempfile::tempdir().unwrap();
    let original = temp.path().join("original");
    let moved = temp.path().join("moved");
    let first = Seat::new(&original).bootstrap().await.unwrap();
    let id = first.identity().to_string();
    assert_eq!(id.len(), 64);
    drop(first);
    assert_eq!(
        Seat::new(&original).bootstrap().await.unwrap().identity(),
        id
    );
    std::fs::rename(&original, &moved).unwrap();
    assert_eq!(Seat::new(&moved).open().await.unwrap().identity(), id);
    let other = Seat::new(temp.path().join("other"))
        .bootstrap()
        .await
        .unwrap();
    assert_ne!(other.identity(), id);
    assert!(!temp.path().join(".concord.lock").exists());
    assert!(!temp.path().join(".issues").exists());
}

#[tokio::test(flavor = "current_thread")]
async fn incomplete() {
    let temp = tempfile::tempdir().unwrap();
    let seat = Seat::new(temp.path());
    let estate = seat.bootstrap().await.unwrap();
    let id = estate.identity().to_string();
    let stale = estate;
    let identity = temp.path().join("workspace/identity.json");
    let bytes = std::fs::read(&identity).unwrap();
    std::fs::remove_file(&identity).unwrap();
    assert_eq!(
        stale.settled().await.unwrap_err().code(),
        "concord.space.identity"
    );
    let error = seat
        .bootstrap()
        .await
        .err()
        .expect("incomplete bootstrap must not repair");
    assert_eq!(error.code(), "concord.space.identity");
    assert!(!identity.exists());
    std::fs::write(&identity, bytes).unwrap();
    assert_eq!(seat.open().await.unwrap().identity(), id);
    std::fs::write(
        &identity,
        serde_json::json!({"schema":"foreign/v1","id":id}).to_string(),
    )
    .unwrap();
    assert_eq!(
        seat.open().await.err().unwrap().code(),
        "concord.space.identity"
    );
}

#[cfg(unix)]
#[tokio::test(flavor = "current_thread")]
async fn redirected() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join("home");
    let foreign = temp.path().join("foreign");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&foreign).unwrap();
    std::os::unix::fs::symlink(&foreign, home.join("workspace")).unwrap();
    let error = Seat::new(&home).bootstrap().await.err().unwrap();
    assert_eq!(error.code(), "concord.space.identity");
    assert_eq!(std::fs::read_dir(&foreign).unwrap().count(), 0);
}
