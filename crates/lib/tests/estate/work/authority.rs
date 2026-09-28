use super::issue_delivery::{Fixture, fixture, released, request, snapshot};
use concord_core::authority::{Authorities, Plumb};
use concord_core::issue_delivery;
use plumb::guard::Authority;

const POINTER: &str = "https://releases.plumb.perish.uk/v1/channels/stable.json";
const UNKNOWN: &str = "v9.9.9@0000000000000000000000000000000000000009";

type Found = Result<Authority, String>;

struct Compiled;
struct Stable;
struct Pinned;
struct Offline;

fn severed() -> Found {
    Err(format!(
        "cannot read Plumb stable Guard authority at {POINTER}: offline"
    ))
}

fn uncompiled() -> Found {
    Err("plumb-lib package carries no released authority".into())
}

impl Authorities for Compiled {
    fn compiled() -> Found {
        Plumb::compiled()
    }

    fn stable() -> Found {
        severed()
    }
}

impl Authorities for Stable {
    fn compiled() -> Found {
        uncompiled()
    }

    fn stable() -> Found {
        Plumb::compiled()
    }
}

impl Authorities for Pinned {
    fn compiled() -> Found {
        Plumb::compiled()
    }

    fn stable() -> Found {
        Plumb::compiled()
    }
}

impl Authorities for Offline {
    fn compiled() -> Found {
        uncompiled()
    }

    fn stable() -> Found {
        severed()
    }
}

#[tokio::test(flavor = "current_thread")]
async fn stable() {
    let authority = released();
    let Fixture {
        estate,
        issue,
        _temp,
    } = fixture(authority.producer(), authority.depot()).await;
    let plan = issue_delivery::prepare::<Stable>(&estate, &request(&issue))
        .await
        .expect("stable authority accepts the proof");
    assert_eq!(plan.authority.producer, authority.producer());
    assert_eq!(plan.authority.depot, authority.depot());
    issue_delivery::revalidate::<Stable>(&estate, &plan, &snapshot(), 2)
        .await
        .expect("stable authority remains accepted");
}

#[tokio::test(flavor = "current_thread")]
async fn unknown() {
    let compiled = released();
    let Fixture {
        estate,
        issue,
        _temp,
    } = fixture(UNKNOWN, compiled.depot()).await;
    let error = issue_delivery::prepare::<Pinned>(&estate, &request(&issue))
        .await
        .expect_err("unknown producer");
    assert_eq!(error.code(), "concord.delivery.authority");
    let accepted = format!("[{0}, {0}]", compiled.producer());
    assert!(error.message().contains(UNKNOWN), "{}", error.message());
    assert!(error.message().contains(&accepted), "{}", error.message());
    assert!(!error.message().contains("unavailable"));
}

#[tokio::test(flavor = "current_thread")]
async fn offline() {
    let compiled = released();
    let Fixture {
        estate,
        issue,
        _temp,
    } = fixture(UNKNOWN, compiled.depot()).await;
    let error = issue_delivery::prepare::<Compiled>(&estate, &request(&issue))
        .await
        .expect_err("compiled mismatch while stable is unreachable");
    assert_eq!(error.code(), "concord.delivery.authority");
    assert!(error.message().contains(POINTER), "{}", error.message());
    assert!(error.message().contains(compiled.producer()));
}

#[tokio::test(flavor = "current_thread")]
async fn moved() {
    let authority = released();
    let Fixture {
        estate,
        issue,
        _temp,
    } = fixture(authority.producer(), authority.depot()).await;
    let plan = issue_delivery::prepare::<Stable>(&estate, &request(&issue))
        .await
        .expect("prepared under stable");
    let error = issue_delivery::revalidate::<Offline>(&estate, &plan, &snapshot(), 2)
        .await
        .expect_err("stable moved after preparation");
    assert_eq!(error.code(), "concord.delivery.authority");
    assert!(error.message().contains("re-guard"), "{}", error.message());
    assert!(error.message().contains(authority.producer()));
    assert!(error.message().contains(POINTER));
}
