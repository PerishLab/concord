use super::pull::{Commit, Pull, State};
use super::settle::merged;

#[test]
fn readback() {
    let exact = pull(State::Merged, Some("merge"));
    assert_eq!(merged(&exact).expect("merged evidence"), "merge");

    let incomplete = pull(State::Open, None);
    assert_eq!(
        merged(&incomplete).expect_err("open is incomplete").code(),
        "concord.delivery.incomplete"
    );

    let closed = pull(State::Closed, None);
    assert_eq!(
        merged(&closed).expect_err("closed is stale").code(),
        "concord.delivery.stale"
    );
}

#[test]
fn evidence() {
    let pull = pull(State::Merged, None);
    assert_eq!(
        merged(&pull).expect_err("missing merge commit").code(),
        "concord.delivery.stale"
    );
}

fn pull(state: State, merge: Option<&str>) -> Pull {
    Pull {
        id: "PR_node".into(),
        number: 7,
        url: "https://github.com/PerishLab/probe/pull/7".into(),
        state,
        base: "main".into(),
        head: "candidate".into(),
        merge: merge.map(|oid| Commit { oid: oid.into() }),
        merged: merge.map(|_| "2026-09-29T00:00:00Z".into()),
        updated: "2026-09-29T00:00:01Z".into(),
        title: "Deliver topic".into(),
        body: "Refs PerishLab/probe#1".into(),
    }
}
