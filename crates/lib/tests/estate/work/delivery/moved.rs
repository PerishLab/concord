use super::completion::merged;
use super::{Fixture, fixture, released, request, snapshot};
use concord_core::authority::{Authorities, Plumb};
use concord_core::delivery;

const UNKNOWN: &str = "v9.9.9@0000000000000000000000000000000000000009";

struct Pinned;

impl Authorities for Pinned {
    fn compiled() -> Result<plumb::guard::Authority, String> {
        Plumb::compiled()
    }

    fn stable() -> Result<plumb::guard::Authority, String> {
        Plumb::compiled()
    }
}

#[tokio::test(flavor = "current_thread")]
async fn moved() {
    let compiled = released();
    let Fixture {
        estate,
        issue,
        _temp,
    } = fixture(compiled.producer(), compiled.depot()).await;
    let snapshot = snapshot();
    let plan = delivery::prepare::<Pinned>(&estate, &request(&issue))
        .await
        .expect("prepare delivery");
    delivery::revalidate::<Pinned>(&estate, &plan, (&snapshot, 2), delivery::Check::Confirm)
        .await
        .expect("Confirm accepts the prepared warrant");
    let mut moved = plan.clone();
    let delivery::Authority::Plumb { warrant, .. } = &mut moved.authority else {
        panic!("expected Guard authority");
    };
    warrant.producer = UNKNOWN.into();
    let refused =
        delivery::revalidate::<Pinned>(&estate, &moved, (&snapshot, 2), delivery::Check::Confirm)
            .await
            .expect_err("a moved warrant refuses before any merge");
    assert_eq!(refused.code(), "concord.delivery.authority");
    assert!(
        refused
            .message()
            .contains("no longer an accepted Guard authority"),
        "{}",
        refused.message()
    );
    let (settlement, integration) = merged(&estate, &issue, &moved, _temp.path()).await;
    let completed = estate
        .complete(&integration, &settlement)
        .await
        .expect("settle the observed merge after the authority moved");
    assert_eq!(completed.revision, 4);
    assert_eq!(completed.head, settlement.merge);
    assert!(completed.released);
    assert_eq!(
        estate
            .member(&issue)
            .await
            .expect_err("Member released")
            .code(),
        "concord.member.absent"
    );
}
