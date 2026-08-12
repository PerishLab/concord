use super::super::scan::Work;
use super::current;
use super::member;
use crate::{Entry, Fact};
use keel::Tx;
use keel::adapt::db::Sqlite;

pub(super) async fn write(
    tx: &mut Tx<'_, Sqlite>,
    work: &Work,
    domain: i64,
) -> std::result::Result<i64, keel::adapt::Error> {
    let domain = domain.to_string();
    let task = tx
        .put(
            "Task",
            &[
                ("name", work.task.name.as_str()),
                ("state", "active"),
                ("revision", "0"),
                ("domain", domain.as_str()),
            ],
        )
        .await?;
    tx.put(
        "Reservation",
        &[
            ("name", work.task.name.as_str()),
            ("domain", domain.as_str()),
        ],
    )
    .await?;
    for held in current(work) {
        fact(tx, task, &held).await?;
    }
    for held in &work.phases {
        phase(tx, task, held).await?;
    }
    for held in &work.task.repo {
        member::write(tx, held, task).await?;
    }
    Ok(task)
}

async fn fact(
    tx: &mut Tx<'_, Sqlite>,
    task: i64,
    fact: &Fact,
) -> std::result::Result<(), keel::adapt::Error> {
    let rank = fact.rank.map(|rank| rank.to_string());
    let task = task.to_string();
    let mut fields = vec![("body", fact.body.as_str()), ("task", task.as_str())];
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

async fn phase(
    tx: &mut Tx<'_, Sqlite>,
    task: i64,
    entries: &[Entry],
) -> std::result::Result<(), keel::adapt::Error> {
    let task = task.to_string();
    let phase = tx.put("Phase", &[("task", task.as_str())]).await?;
    let phase = phase.to_string();
    for entry in entries {
        let rank = entry.rank.map(|rank| rank.to_string());
        let mut fields = vec![("body", entry.body.as_str()), ("phase", phase.as_str())];
        if let Some(rank) = rank.as_deref() {
            fields.push(("rank", rank));
        }
        if let Some(title) = entry.title.as_deref() {
            fields.push(("title", title));
        }
        if let Some(origin) = entry.origin.as_deref() {
            fields.push(("origin", origin));
        }
        tx.put(entry.part.unit(), &fields).await?;
    }
    Ok(())
}
