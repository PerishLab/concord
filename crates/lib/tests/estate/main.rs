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
    std::fs::create_dir_all(temp.path().join(".concord")).expect("estate root");
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
    assert!(!temp.path().join(".concord").exists());
}
