use super::git::Git;
use super::pull::{Client, Report, stale};
use concord_core::{Error, Result};
use plumb::delivery::{Squash, landed};
use plumb::landing::Preparation;
use std::path::Path;

pub async fn merge(
    report: &mut Report,
    preparation: &Preparation,
    command: &Path,
    timeout: u64,
) -> Result<()> {
    Git::fetch(&preparation.root).await?;
    let source = Git::run(&preparation.root, &["rev-parse", "HEAD"]).await?;
    let target = Git::run(
        &preparation.root,
        &["rev-parse", &format!("origin/{}", preparation.base)],
    )
    .await?;
    if source != preparation.source || target != preparation.target {
        return Err(stale(
            "source or base advanced after the exact pull was created",
        ));
    }
    let repository = Git::repository(&preparation.root).await?;
    let mut client = Client {
        command,
        repository: &repository,
        timeout,
    };
    let squash = Squash::read(&preparation.root, &preparation.candidate).map_err(|refusal| {
        Error::typed(
            "concord.delivery.squash",
            format!(
                "cannot derive the squash for candidate {}: {refusal}",
                preparation.candidate
            ),
        )
    })?;
    client.mark(&preparation.candidate).await?;
    client.settle(report.pull.number, &squash).await?;
    readback(preparation, report.pull.number).await?;
    report.merged = true;
    Ok(())
}

async fn readback(preparation: &Preparation, number: i64) -> Result<()> {
    Git::fetch(&preparation.root).await?;
    let base = format!("origin/{}", preparation.base);
    let head = Git::run(&preparation.root, &["rev-parse", "--verify", &base]).await?;
    let candidate = &preparation.candidate;
    landed(&preparation.root, candidate, &head)
        .map(|_| ())
        .map_err(|refusal| {
            Error::typed(
                "concord.delivery.landed",
                format!(
                    "pull {number} merged, but {base} head {head} does not hold candidate {candidate} ({}): {refusal}",
                    refusal.kind
                ),
            )
        })
}
