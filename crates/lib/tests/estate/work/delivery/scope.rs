use super::{fixture, git, released, request, snapshot};
use concord_core::authority::Plumb;
use concord_core::{Register, Repository, delivery};

async fn foreign(estate: &concord_core::Estate) -> super::super::start::World {
    let peer = super::super::start::world().await;
    git(
        &peer.source,
        &[
            "remote",
            "set-url",
            "origin",
            "https://github.com/PerishLab/foreign.git",
        ],
    );
    estate
        .register(&Register {
            node: "R_foreign".into(),
            repository: Repository::parse("PerishLab/foreign").unwrap(),
            path: peer.source.clone(),
        })
        .await
        .unwrap();
    peer
}

#[tokio::test(flavor = "current_thread")]
async fn reentry() {
    let authority = released();
    let world = fixture(authority.producer(), authority.depot()).await;
    let peer = foreign(&world.estate).await;
    std::fs::write(peer.source.join("foreign"), "preserve").unwrap();
    let plan = delivery::prepare::<Plumb>(&world.estate, &request(&world.issue))
        .await
        .unwrap();
    std::fs::rename(&peer.source, peer.temp.path().join("retained")).unwrap();
    delivery::resume::<Plumb>(&world.estate, &plan, &snapshot())
        .await
        .unwrap();
    delivery::revalidate::<Plumb>(
        &world.estate,
        &plan,
        (&snapshot(), 2),
        delivery::Check::Confirm,
    )
    .await
    .unwrap();
    let source = world._temp.path().join("source");
    std::fs::write(source.join("changed"), "preserve target").unwrap();
    let error = delivery::revalidate::<Plumb>(
        &world.estate,
        &plan,
        (&snapshot(), 3),
        delivery::Check::Confirm,
    )
    .await
    .unwrap_err();
    assert_eq!(error.code(), "concord.audit.refused");
    assert_eq!(
        std::fs::read_to_string(peer.temp.path().join("retained/foreign")).unwrap(),
        "preserve"
    );
    assert_eq!(world.estate.issue(&world.issue).await.unwrap().revision, 2);
}

#[tokio::test(flavor = "current_thread")]
async fn completion() {
    let authority = released();
    let world = fixture(authority.producer(), authority.depot()).await;
    let plan = delivery::prepare::<Plumb>(&world.estate, &request(&world.issue))
        .await
        .unwrap();
    let peer = foreign(&world.estate).await;
    let (settlement, integration) =
        super::completion::merged(&world.estate, &world.issue, &plan, world._temp.path()).await;
    std::fs::rename(&peer.source, peer.temp.path().join("retained")).unwrap();
    let completed = world
        .estate
        .complete(&integration, &settlement)
        .await
        .unwrap();
    assert!(completed.released);
    assert_eq!(completed.head, settlement.merge);
    assert!(
        delivery::resume::<Plumb>(&world.estate, &plan, &snapshot())
            .await
            .unwrap()
            .released
    );
    assert!(!world.estate.inspect().await.unwrap().agrees());
    assert!(peer.temp.path().join("retained/README.md").is_file());
}
