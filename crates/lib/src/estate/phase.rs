use super::World;
use super::current::{prepare, write};
use super::{Current, Entry, Estate, Life, Part, Phase, Settle, Settlement, fault};
use crate::{Error, Result};
use keel::adapt::db::Sqlite;
use keel::{Row, Tx};
use std::collections::BTreeSet;

impl Estate {
    pub async fn settle(&self, settle: &Settle) -> Result<Settlement> {
        validate(settle)?;
        let _guard = self.guard()?;
        self.ensure().await?;
        let world = World::load(self).await?;
        let task = world.node(&settle.task)?;
        active(task.life, &settle.task)?;
        if task.revision != settle.revision {
            return Err(Error::typed(
                "concord.task.stale",
                format!(
                    "Task revision changed: expected {}, found {}",
                    settle.revision, task.revision
                ),
            ));
        }
        let current = self.facts(task.key).await?;
        let prepared = prepare::build(&current, &settle.edits)?;
        let temporary = prepare::temporary(&current, &prepared)?;
        let revision = task.revision + 1;
        let next = revision.to_string();
        let root = task.key;
        let phase = self
            .core
            .batch(async |tx| {
                let phase = create(tx, root, &settle.phase).await?;
                write::apply(tx, root, &prepared, &temporary).await?;
                tx.set("Task", root, &[("revision", next.as_str())]).await?;
                Ok(phase)
            })
            .await
            .map_err(fault)?;
        let mut task = task.clone();
        task.revision = revision;
        Ok(Settlement {
            current: Current {
                task,
                facts: self.facts(root).await?,
                reference: self.forge(root, None).await?,
            },
            phase: self.phase(phase).await?,
        })
    }

    #[locus::trace(with = crate::observation::view())]
    pub async fn phases(&self, identity: &str) -> Result<Vec<Phase>> {
        let world = World::load(self).await?;
        let task = world.node(identity)?;
        let mut phases = Vec::new();
        for row in self.core.live("Phase").await.map_err(fault)? {
            if row.int("task") == Some(task.key) {
                phases.push(self.phase(row.key()).await?);
            }
        }
        phases.sort_by_key(|phase| phase.number);
        Ok(phases)
    }

    async fn phase(&self, key: i64) -> Result<Phase> {
        let row = self
            .core
            .live("Phase")
            .await
            .map_err(fault)?
            .into_iter()
            .find(|row| row.key() == key)
            .ok_or_else(|| {
                Error::typed("concord.phase.absent", format!("Phase {key} not found"))
            })?;
        let number = row.int("number").ok_or_else(|| malformed(&row))?;
        let mut entries = Vec::new();
        for part in Part::ALL {
            for row in self.core.live(part.unit()).await.map_err(fault)? {
                if row.int("phase") == Some(key) {
                    entries.push(read(part, &row)?);
                }
            }
        }
        entries.sort_by_key(|entry| (entry.part, entry.rank.unwrap_or(0), entry.key.unwrap_or(0)));
        audit(&entries, false)?;
        Ok(Phase {
            key,
            number,
            entries,
        })
    }
}

fn validate(settle: &Settle) -> Result<()> {
    if settle.version != 1 {
        return Err(Error::typed(
            "concord.settle.version",
            format!("unsupported settle version {}", settle.version),
        ));
    }
    audit(&settle.phase, true)
}

fn audit(entries: &[Entry], fresh: bool) -> Result<()> {
    let outcomes = entries
        .iter()
        .filter(|entry| entry.part == Part::Outcome)
        .count();
    if fresh && outcomes != 1 {
        return Err(Error::typed(
            "concord.phase.outcome",
            "new Phase requires exactly one Outcome",
        ));
    }
    if !fresh && outcomes > 1 {
        return Err(Error::typed(
            "concord.phase.outcome",
            "Phase admits at most one Outcome",
        ));
    }
    let mut ranks = BTreeSet::new();
    for entry in entries {
        shape(entry, fresh)?;
        if let Some(rank) = entry.rank
            && !ranks.insert((entry.part, rank))
        {
            return Err(Error::typed(
                "concord.phase.rank",
                format!("{:?} ranks must be unique", entry.part),
            ));
        }
    }
    Ok(())
}

fn shape(entry: &Entry, fresh: bool) -> Result<()> {
    if fresh != entry.key.is_none() {
        return Err(Error::typed(
            "concord.phase.key",
            "new Phase entries omit keys and retained entries carry keys",
        ));
    }
    if entry.body.trim().is_empty() {
        return Err(Error::typed(
            "concord.phase.body",
            "Phase entry body cannot be blank",
        ));
    }
    if entry.part.singular() != entry.rank.is_none() {
        return Err(Error::typed(
            "concord.phase.rank",
            "Outcome omits rank and ordered Phase entries require rank",
        ));
    }
    if entry.rank.is_some_and(|rank| rank < 1) {
        return Err(Error::typed(
            "concord.phase.rank",
            "Phase entry rank must be positive",
        ));
    }
    let addition = entry.part == Part::Addition;
    if addition
        && entry
            .origin
            .as_ref()
            .is_none_or(|origin| origin.trim().is_empty())
    {
        return Err(Error::typed(
            "concord.phase.origin",
            "Phase Addition requires a nonblank origin",
        ));
    }
    if !addition && (entry.title.is_some() || entry.origin.is_some()) {
        return Err(Error::typed(
            "concord.phase.shape",
            "title and origin belong only to Phase Addition",
        ));
    }
    Ok(())
}

async fn create(
    tx: &mut Tx<'_, Sqlite>,
    task: i64,
    entries: &[Entry],
) -> std::result::Result<i64, keel::adapt::Error> {
    let root = task.to_string();
    let phase = tx.put("Phase", &[("task", root.as_str())]).await?;
    for entry in entries {
        child(tx, phase, entry).await?;
    }
    Ok(phase)
}

async fn child(
    tx: &mut Tx<'_, Sqlite>,
    phase: i64,
    entry: &Entry,
) -> std::result::Result<(), keel::adapt::Error> {
    let root = phase.to_string();
    let rank = entry.rank.map(|rank| rank.to_string());
    let mut fields = vec![("body", entry.body.as_str()), ("phase", root.as_str())];
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
    Ok(())
}

fn read(part: Part, row: &Row) -> Result<Entry> {
    Ok(Entry {
        key: Some(row.key()),
        part,
        rank: row.int("rank"),
        title: row.text("title").map(str::to_string),
        body: row.text("body").unwrap_or("").to_string(),
        origin: row.text("origin").map(str::to_string),
    })
}

fn active(life: Life, identity: &str) -> Result<()> {
    if life == Life::Active {
        return Ok(());
    }
    Err(Error::typed(
        "concord.task.retired",
        format!("retired Task is immutable: {identity}"),
    ))
}

fn malformed(row: &Row) -> Error {
    Error::typed(
        "concord.phase.row",
        format!("Phase {} has malformed number", row.key()),
    )
}
