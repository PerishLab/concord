use super::git::Git;
use super::pull::{Pull, State, stale};
use concord_core::issue_delivery::Preparation;
use concord_core::{Error, Result};
use plumb::delivery::Squash;

pub async fn exact(preparation: &Preparation) -> Result<()> {
    Git::fetch(&preparation.root).await?;
    let source = Git::run(&preparation.root, &["rev-parse", "HEAD"]).await?;
    let target = Git::run(
        &preparation.root,
        &["rev-parse", &format!("origin/{}", preparation.base)],
    )
    .await?;
    if source != preparation.source || target != preparation.target {
        return Err(stale(
            "source or base advanced before the next provider mutation",
        ));
    }
    Ok(())
}

pub fn squash(preparation: &Preparation) -> Result<Squash> {
    Squash::read(&preparation.root, &preparation.candidate).map_err(|refusal| {
        Error::typed(
            "concord.delivery.squash",
            format!(
                "cannot derive the squash for candidate {}: {refusal}",
                preparation.candidate
            ),
        )
    })
}

pub fn merged(pull: &Pull) -> Result<String> {
    match pull.state {
        State::Merged => pull
            .merge
            .as_ref()
            .map(|commit| commit.oid.clone())
            .filter(|commit| !commit.is_empty())
            .ok_or_else(|| stale("merged pull has no merge commit")),
        State::Closed => Err(stale("exact pull closed without merging")),
        State::Open => Err(Error::typed(
            "concord.delivery.incomplete",
            "merge command returned without merged-state provider readback",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::super::pull::{Commit, Pull, State};
    use super::merged;

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
}
