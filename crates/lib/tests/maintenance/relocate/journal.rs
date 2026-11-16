use super::{Intent, Ready, Relocate, Result, files, payload, refuse};
use radicula_lock::{
    Policy, Wait,
    file::{Gate, Permit},
};
use serde::{Serialize, de::DeserializeOwned};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

pub(super) fn root(home: &Path) -> PathBuf {
    home.join("state/relocate")
}
pub(super) fn stage(home: &Path) -> PathBuf {
    root(home).join("stage")
}
impl Relocate {
    pub(super) fn frozen(&self) -> PathBuf {
        self.source.join(".concord/sudo.offline")
    }
}
pub(super) fn possession(home: &Path) -> PathBuf {
    root(home).join("possession")
}

pub(super) fn write(path: &Path, value: &impl Serialize) -> Result<()> {
    let bytes = serde_json::to_vec(value).map_err(|error| refuse(error.to_string()))?;
    let pending = path.with_extension("pending");
    if path.symlink_metadata().is_ok() {
        return Err(refuse("journal entry already exists"));
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&pending)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    drop(file);
    fs::rename(pending, path)?;
    Ok(())
}

pub(super) fn read<T: DeserializeOwned>(path: &Path) -> Result<T> {
    files::digest(path)?;
    serde_json::from_slice(&fs::read(path)?).map_err(|error| refuse(error.to_string()))
}

pub(super) fn intent(source: &Path, home: &Path) -> Result<Intent> {
    files::directory(&root(home))?;
    let value: Intent = read(&root(home).join("intent.json"))?;
    if value.schema != "concord.member-relocation/v1"
        || value.source != source
        || value.home != home
    {
        return Err(refuse(
            "journal does not bind this exact source/home/schema",
        ));
    }
    value.files.check()?;
    Ok(value)
}

impl Ready {
    pub(super) fn read(home: &Path) -> Result<Self> {
        read(&root(home).join("ready.json"))
    }
}

pub(super) fn controls(intent: &Intent) -> Result<()> {
    if intent
        .source
        .join(".concord/sudo")
        .symlink_metadata()
        .is_ok()
    {
        return Err(refuse("old authority reappeared"));
    }
    let mut current = files::inventory(&intent.source.join(".concord"))?;
    let sudo = current
        .files
        .remove(Path::new("sudo.offline"))
        .ok_or_else(|| refuse("missing frozen authority"))?;
    current.files.insert("sudo".into(), sudo);
    if current != intent.files {
        return Err(refuse("frozen controls changed"));
    }
    Ok(())
}

pub(super) fn payload(source: &Path) -> Result<Option<payload::Payload>> {
    let path = source.join(".issues");
    if path.symlink_metadata().is_ok() {
        Ok(Some(payload::inventory(&path)?))
    } else {
        Ok(None)
    }
}

pub(super) fn unchanged(intent: &Intent) -> Result<()> {
    controls(intent)?;
    if payload(&intent.source)? != intent.payload {
        return Err(refuse("source payload changed"));
    }
    Ok(())
}

pub(super) fn hold(source: &Path) -> Result<Vec<Permit>> {
    if !source.join(".concord").try_exists()? {
        return Ok(Vec::new());
    }
    files::directory(&source.join(".concord"))?;
    let mut paths = vec![source.join(".concord.lock")];
    for entry in fs::read_dir(source.join(".concord"))? {
        let path = entry?.path();
        if path.extension().is_some_and(|value| value == "lock") {
            paths.push(path);
        }
    }
    paths.sort();
    let mut held = Vec::new();
    for path in paths {
        if path.symlink_metadata().is_ok() {
            files::digest(&path)?;
        }
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;
        let gate = Gate::new(file, Policy::EXCLUSIVE).map_err(|error| refuse(error.to_string()))?;
        held.push(
            gate.acquire(Wait::immediate())
                .map_err(|_| refuse("legacy writer lock is occupied"))?,
        );
    }
    Ok(held)
}

pub(super) fn release(held: Vec<Permit>, outcome: Result<()>) -> Result<()> {
    let failures = held
        .into_iter()
        .filter_map(|permit| permit.release().err())
        .collect::<Vec<_>>();
    if !failures.is_empty() {
        return Err(refuse(
            "lock release failed; preserve outcome and migration state",
        ));
    }
    outcome
}
