use super::super::{Fact, Role};
use super::prepare::Prepared;
use keel::Tx;
use keel::adapt::db::Sqlite;
use std::collections::BTreeMap;

pub(in crate::estate) async fn apply(
    tx: &mut Tx<'_, Sqlite>,
    task: i64,
    prepared: &Prepared,
    temporary: &BTreeMap<(Role, i64), i64>,
) -> Result<(), keel::adapt::Error> {
    for (role, key) in &prepared.end {
        tx.end(role.unit(), *key).await?;
    }
    for fact in &prepared.set {
        let key = fact.key.expect("set key");
        if let Some(rank) = temporary.get(&(fact.role, key)) {
            let value = rank.to_string();
            tx.set(fact.role.unit(), key, &[("rank", &value)]).await?;
        }
    }
    for fact in &prepared.create {
        create(tx, task, fact).await?;
    }
    for fact in &prepared.set {
        update(tx, fact).await?;
    }
    Ok(())
}

async fn create(tx: &mut Tx<'_, Sqlite>, task: i64, fact: &Fact) -> Result<(), keel::adapt::Error> {
    let root = task.to_string();
    let rank = fact.rank.map(|rank| rank.to_string());
    let mut fields = vec![("body", fact.body.as_str()), ("task", root.as_str())];
    if let Some(rank) = rank.as_deref() {
        fields.push(("rank", rank));
    }
    if let Some(title) = fact.title.as_deref() {
        fields.push(("title", title));
    }
    if let Some(origin) = fact.origin.as_deref() {
        fields.push(("origin", origin));
    }
    tx.put(fact.role.unit(), &fields).await?;
    Ok(())
}

async fn update(tx: &mut Tx<'_, Sqlite>, fact: &Fact) -> Result<(), keel::adapt::Error> {
    let key = fact.key.expect("set key");
    let rank = fact.rank.map(|rank| rank.to_string());
    let mut fields = vec![("body", fact.body.as_str())];
    if let Some(rank) = rank.as_deref() {
        fields.push(("rank", rank));
    }
    if let Some(origin) = fact.origin.as_deref() {
        fields.push(("origin", origin));
    }
    if let Some(title) = fact.title.as_deref() {
        fields.push(("title", title));
        tx.set(fact.role.unit(), key, &fields).await
    } else {
        tx.set(fact.role.unit(), key, &fields).await?;
        if fact.role == Role::Addition {
            tx.unset(fact.role.unit(), key, &["title"]).await?;
        }
        Ok(())
    }
}
