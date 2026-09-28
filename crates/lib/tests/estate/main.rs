mod work;

use concord_core::Seat;
use keel::adapt::db::Sqlite;
use keel::wire::Wire;

#[tokio::test(flavor = "current_thread")]
async fn genesis() {
    let temp = tempfile::tempdir().expect("temporary Space");
    let seat = Seat::new(temp.path());
    let estate = seat.bootstrap().await.expect("bootstrap Issue estate");
    assert!(estate.inspect(None, None).await.expect("audit").agrees());
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
