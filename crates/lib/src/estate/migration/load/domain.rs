use super::super::scan::Realm;
use super::extra::{self, Parent};
use super::task;
use crate::Repo;
use keel::Tx;
use keel::adapt::db::Sqlite;
use std::collections::BTreeMap;

pub(super) async fn seed(
    tx: &mut Tx<'_, Sqlite>,
    realm: &Realm,
    space: &str,
    tasks: &mut BTreeMap<String, i64>,
) -> std::result::Result<(), keel::adapt::Error> {
    let domain = tx
        .put(
            "Domain",
            &[
                ("name", realm.name.as_str()),
                ("revision", "0"),
                ("space", space),
            ],
        )
        .await?;
    extra::write(
        tx,
        Parent::new("domain:addition", "domain", domain),
        &realm.registry.extra,
    )
    .await?;
    for held in &realm.registry.repo {
        repository(tx, held, domain).await?;
    }
    for held in &realm.works {
        let key = task::write(tx, held, domain).await?;
        tasks.insert(format!("{}/{}", realm.name, held.task.name), key);
    }
    Ok(())
}

async fn repository(
    tx: &mut Tx<'_, Sqlite>,
    repository: &Repo,
    domain: i64,
) -> std::result::Result<(), keel::adapt::Error> {
    let domain = domain.to_string();
    let mut fields = vec![
        ("name", repository.name.as_str()),
        ("domain", domain.as_str()),
    ];
    if let Some(note) = repository.note.as_deref() {
        fields.push(("note", note));
    }
    let key = tx.put("Repository", &fields).await?;
    extra::write(
        tx,
        Parent::new("repository:addition", "repository", key),
        &repository.extra,
    )
    .await
}
