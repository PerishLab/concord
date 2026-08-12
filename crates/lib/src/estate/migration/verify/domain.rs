use super::super::super::{Estate, fault};
use super::super::scan::Source;
use super::extra;
use super::mismatch;
use crate::Result;

pub(super) async fn check(estate: &Estate, source: &Source) -> Result<()> {
    let domains = estate.core.live("Domain").await.map_err(fault)?;
    let repositories = estate.core.live("Repository").await.map_err(fault)?;
    let additions = estate.core.live("domain:addition").await.map_err(fault)?;
    let notes = estate
        .core
        .live("repository:addition")
        .await
        .map_err(fault)?;
    for realm in &source.realms {
        let domain = domains
            .iter()
            .find(|row| row.text("name") == Some(realm.name.as_str()))
            .ok_or_else(|| mismatch(format!("missing Domain {}", realm.name)))?;
        extra::check(&additions, "domain", domain.key(), &realm.registry.extra)?;
        for repository in &realm.registry.repo {
            let held = repositories
                .iter()
                .find(|row| {
                    row.int("domain") == Some(domain.key())
                        && row.text("name") == Some(repository.name.as_str())
                })
                .ok_or_else(|| mismatch(format!("missing Repository {}", repository.name)))?;
            if held.text("note") != repository.note.as_deref() {
                return Err(mismatch(format!(
                    "Repository note differs: {}/{}",
                    realm.name, repository.name
                )));
            }
            extra::check(&notes, "repository", held.key(), &repository.extra)?;
        }
    }
    Ok(())
}
