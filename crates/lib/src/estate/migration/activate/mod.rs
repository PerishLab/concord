mod journal;

use super::super::{Estate, Seat};
use super::{Activation, RELEASE, Staging, agreement, scan, verify};
use crate::path::at;
use crate::{Error, Result, Space};
use journal::Journal;
use std::ffi::OsString;
use std::path::Path;

const MARKER: &str = "version = 4\nmigration = \"v0.10.0\"\n";

pub(super) async fn run(staging: Staging, legacy: &Space, fingerprint: &str) -> Result<Activation> {
    if fingerprint != staging.census.fingerprint {
        return Err(Error::typed(
            "concord.migration.expect",
            "activation fingerprint differs from staged Census",
        ));
    }
    agreement(legacy)?;
    let seat = Seat::new(legacy.path());
    seat.same(legacy)?;
    let _guard = legacy.lock()?;
    agreement(legacy)?;
    let source = scan::Source::read(legacy)?;
    if source.census != staging.census {
        return Err(Error::typed(
            "concord.migration.changed",
            "legacy Space changed after staging",
        ));
    }
    let Staging {
        root,
        census,
        estate,
    } = staging;
    drop(estate);
    let rollback = seat.root.join("migration").join(RELEASE).join("rollback");
    preflight(&seat, &source, &root, &rollback)?;
    at(&rollback).directory()?;
    let manifest = serde_json::to_string_pretty(&census)
        .map_err(|error| Error::new(error.to_string()))?
        + "\n";
    at(&rollback.join("MANIFEST.json")).file(&manifest)?;
    let mut journal = Journal::new();
    let transition = Transition {
        seat: &seat,
        source: &source,
        stage: &root,
        rollback: &rollback,
    };
    if let Err(error) = apply(&transition, &mut journal) {
        return failed(error, &journal);
    }
    let active = match seat.open().await {
        Ok(active) => active,
        Err(error) => return failed(error, &journal),
    };
    if let Err(error) = verify::check(&active, &source).await {
        drop(active);
        return failed(error, &journal);
    }
    if let Err(error) = payload(&active, &source).await {
        drop(active);
        return failed(error, &journal);
    }
    let _ = std::fs::remove_dir(&root);
    Ok(Activation {
        root: rollback,
        census,
        estate: active,
    })
}

fn preflight(seat: &Seat, source: &scan::Source, stage: &Path, rollback: &Path) -> Result<()> {
    if seat.database().exists() || seat.sudo().exists() || rollback.exists() {
        return Err(Error::typed(
            "concord.migration.target",
            "activation target or rollback seat is occupied",
        ));
    }
    let mut held = std::fs::read_dir(stage)?
        .map(|entry| entry.map(|entry| entry.file_name()).map_err(Error::from))
        .collect::<Result<Vec<_>>>()?;
    held.sort();
    let mut wanted = vec![OsString::from("estate.sqlite3"), OsString::from("sudo")];
    wanted.sort();
    if held != wanted {
        return Err(Error::typed(
            "concord.migration.stage",
            "staged estate contains unexpected files",
        ));
    }
    for realm in &source.realms {
        let registry = seat.space.join(&realm.name).join(".tasks/tasks.toml");
        if !registry.is_file() {
            return Err(Error::typed(
                "concord.migration.registry",
                format!("legacy registry disappeared: {}", registry.display()),
            ));
        }
        for work in &realm.works {
            let memory = seat
                .space
                .join(&realm.name)
                .join(".tasks")
                .join(&work.task.name)
                .join(".task");
            let resources = memory.join("resources");
            if resources.exists() && memory.join("artifacts").exists() {
                return Err(Error::typed(
                    "concord.migration.artifact",
                    format!("Artifact target is occupied: {}", memory.display()),
                ));
            }
        }
    }
    Ok(())
}

struct Transition<'a> {
    seat: &'a Seat,
    source: &'a scan::Source,
    stage: &'a Path,
    rollback: &'a Path,
}

fn apply(transition: &Transition<'_>, journal: &mut Journal) -> Result<()> {
    for realm in &transition.source.realms {
        let registry = transition
            .seat
            .space
            .join(&realm.name)
            .join(".tasks/tasks.toml");
        let archive = transition
            .rollback
            .join("domains")
            .join(&realm.name)
            .join("tasks.toml");
        journal.shift(&registry, &archive)?;
        journal.marker(&registry, MARKER)?;
    }
    for realm in &transition.source.realms {
        for work in &realm.works {
            let task = transition
                .seat
                .space
                .join(&realm.name)
                .join(".tasks")
                .join(&work.task.name);
            let archive = transition
                .rollback
                .join("tasks")
                .join(&realm.name)
                .join(&work.task.name);
            memory(&task, &archive, journal)?;
            journal.prune(&task)?;
        }
    }
    journal.shift(
        &transition.stage.join("estate.sqlite3"),
        &transition.seat.database(),
    )?;
    journal.shift(&transition.stage.join("sudo"), &transition.seat.sudo())?;
    Ok(())
}

fn memory(task: &Path, archive: &Path, journal: &mut Journal) -> Result<()> {
    let memory = task.join(".task");
    if !memory.exists() {
        return Ok(());
    }
    let main = memory.join("MAIN.md");
    if main.exists() {
        journal.shift(&main, &archive.join("MAIN.md"))?;
    }
    let phases = memory.join("phases");
    if phases.exists() {
        journal.shift(&phases, &archive.join("phases"))?;
    }
    let resources = memory.join("resources");
    let artifacts = memory.join("artifacts");
    if resources.exists() {
        journal.shift(&resources, &artifacts)?;
        journal.prune(&artifacts)?;
    }
    journal.prune(&memory)
}

async fn payload(estate: &Estate, source: &scan::Source) -> Result<()> {
    let mut artifacts = 0;
    for realm in &source.realms {
        for work in &realm.works {
            artifacts += residue(estate, &realm.name, work).await?;
        }
    }
    if artifacts != source.census.artifact {
        return Err(Error::typed(
            "concord.migration.mismatch",
            format!(
                "Artifact count differs: expected {}, found {artifacts}",
                source.census.artifact
            ),
        ));
    }
    Ok(())
}

async fn residue(estate: &Estate, domain: &str, work: &scan::Work) -> Result<usize> {
    let identity = format!("{domain}/{}", work.task.name);
    let artifacts = estate.artifacts(&identity).await?.len();
    let memory = estate
        .space
        .join(domain)
        .join(".tasks")
        .join(&work.task.name)
        .join(".task");
    for legacy in [
        memory.join("MAIN.md"),
        memory.join("phases"),
        memory.join("resources"),
    ] {
        if legacy.exists() {
            return Err(Error::typed(
                "concord.migration.payload",
                format!("legacy payload remains active: {}", legacy.display()),
            ));
        }
    }
    Ok(artifacts)
}

fn failed(error: Error, journal: &Journal) -> Result<Activation> {
    match journal.rollback() {
        Ok(()) => Err(error),
        Err(rollback) => Err(Error::typed(
            "concord.migration.disagreement",
            format!("{error}; rollback failed: {rollback}"),
        )),
    }
}
