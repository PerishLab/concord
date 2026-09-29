use super::git::Git;
use super::pull::{Pull, State, stale};
use concord_core::{Error, Result};
use plumb::delivery::Squash;
use plumb::landing::Preparation;

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
