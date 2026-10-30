use super::{fixture, released, request};
use concord_core::authority::Plumb;
use concord_core::issue_delivery::{self, execution};
use concord_core::{Admission, Coordinate};
use std::future::Future;

#[tokio::test(flavor = "current_thread")]
async fn independent() {
    let authority = released();
    let world = fixture(authority.producer(), authority.depot()).await;
    let plan = issue_delivery::prepare::<Plumb>(&world.estate, &request(&world.issue))
        .await
        .expect("plan");
    let permit = execution::acquire(&world.estate, &plan)
        .await
        .expect("first");
    let refusal = execution::acquire(&world.estate, &plan)
        .await
        .err()
        .expect("contender");
    assert_eq!(refusal.code(), "concord.delivery.running");
    assert_eq!(
        refusal.details().unwrap()["holder"]["pid"],
        std::process::id()
    );
    let other = Coordinate::parse("PerishLab/probe#2").unwrap();
    world
        .estate
        .admit(&Admission {
            node: "I_peer".into(),
            coordinate: other.clone(),
        })
        .await
        .expect("peer Anchor");
    let mut peer = plan.clone();
    peer.issue = other;
    peer.node = "I_peer".into();
    execution::acquire(&world.estate, &peer)
        .await
        .expect("independent Issue")
        .release()
        .expect("release peer");
    let integration = plan
        .member
        .integration
        .guard(&world.estate)
        .expect("Integration is not held by invocation admission");
    drop(integration);
    assert!(
        world
            .estate
            .settled()
            .await
            .expect("read-only agreement")
            .agrees()
    );
    let mut wrong = plan.clone();
    wrong.node = "I_wrong".into();
    assert_eq!(
        execution::acquire(&world.estate, &wrong)
            .await
            .err()
            .expect("wrong identity")
            .code(),
        "concord.delivery.coordinate"
    );
    permit.release().expect("release");
    execution::acquire(&world.estate, &plan)
        .await
        .expect("stale diagnostic does not block")
        .release()
        .expect("release resumed");
}

#[tokio::test(flavor = "current_thread")]
async fn cancellation() {
    let authority = released();
    let world = fixture(authority.producer(), authority.depot()).await;
    let plan = issue_delivery::prepare::<Plumb>(&world.estate, &request(&world.issue))
        .await
        .expect("plan");
    let permit = execution::acquire(&world.estate, &plan)
        .await
        .expect("admission");
    let mut pending = Box::pin(permit.during(std::future::pending::<()>));
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    assert!(pending.as_mut().poll(&mut context).is_pending());
    assert_eq!(
        execution::acquire(&world.estate, &plan)
            .await
            .err()
            .expect("held across suspension")
            .code(),
        "concord.delivery.running"
    );
    drop(pending);
    let permit = execution::acquire(&world.estate, &plan)
        .await
        .expect("cancelled scope released");
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        permit.run(|| panic!("injected unwind"))
    }));
    assert!(panic.is_err());
    execution::acquire(&world.estate, &plan)
        .await
        .expect("unwound scope released")
        .release()
        .expect("release");
}

#[cfg(unix)]
#[tokio::test(flavor = "current_thread")]
async fn redirected() {
    let authority = released();
    let world = fixture(authority.producer(), authority.depot()).await;
    let plan = issue_delivery::prepare::<Plumb>(&world.estate, &request(&world.issue))
        .await
        .expect("plan");
    let anchor = world.estate.issue(&world.issue).await.expect("Anchor");
    let path = world
        ._temp
        .path()
        .join(".concord")
        .join(format!("land-{}.lock", anchor.key));
    let payload = world._temp.path().join("valuable");
    std::fs::write(&payload, "preserved").expect("payload");
    std::os::unix::fs::symlink(&payload, &path).expect("redirected lock");
    assert_eq!(
        execution::acquire(&world.estate, &plan)
            .await
            .err()
            .expect("preserve redirect")
            .code(),
        "concord.delivery.execution"
    );
    assert_eq!(std::fs::read_to_string(payload).unwrap(), "preserved");
}

#[tokio::test(flavor = "current_thread")]
async fn publication() {
    let authority = released();
    let world = fixture(authority.producer(), authority.depot()).await;
    let plan = issue_delivery::prepare::<Plumb>(&world.estate, &request(&world.issue))
        .await
        .expect("plan");
    let anchor = world.estate.issue(&world.issue).await.expect("Anchor");
    let path = world
        ._temp
        .path()
        .join(".concord")
        .join(format!("land-{}.json", anchor.key));
    std::fs::create_dir(&path).expect("inject occupied diagnostic path");
    assert_eq!(
        execution::acquire(&world.estate, &plan)
            .await
            .err()
            .expect("publication refuses")
            .code(),
        "concord.delivery.execution"
    );
    assert!(path.is_dir(), "unexpected path is preserved");
    std::fs::remove_dir(path).expect("remove only injected fixture directory");
    execution::acquire(&world.estate, &plan)
        .await
        .expect("failed publication released kernel admission")
        .release()
        .expect("release");
}
