use super::{Cutover, Error, Intent, Result, files, refuse};
use radicula_lock::{
    Policy, Wait,
    file::{Gate, Permit},
};
use serde::{Serialize, de::DeserializeOwned};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

pub(super) fn root(home: &Path) -> PathBuf {
    home.join("state/cutover")
}
pub(super) fn frozen(source: &Path) -> PathBuf {
    source.join(".concord/sudo.offline")
}
pub(super) fn stage(home: &Path) -> PathBuf {
    root(home).join("stage")
}

pub(super) fn write<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let bytes = serde_json::to_vec(value).map_err(|error| refuse(error.to_string()))?;
    if path.symlink_metadata().is_ok() {
        return Err(refuse("journal entry already exists"));
    }
    let pending = path.with_extension("pending");
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
    let intent: Intent = read(&root(home).join("intent.json"))?;
    if intent.schema != "concord.offline-cutover/v1"
        || intent.source != source
        || intent.home != home
    {
        return Err(refuse("journal identity does not match exact source/home"));
    }
    intent.files.check()?;
    if let Some(artifacts) = &intent.artifacts {
        artifacts.check()?;
    }
    Ok(intent)
}

pub(super) fn unchanged(intent: &Intent) -> Result<()> {
    let source = &intent.source;
    if source.join(".concord/sudo").try_exists()? || !frozen(source).try_exists()? {
        return Err(refuse(
            "source authority is not frozen; refuse activation/replay",
        ));
    }
    let mut current = files::inventory(&source.join(".concord"))?;
    let sudo = current
        .files
        .remove(Path::new("sudo.offline"))
        .ok_or_else(|| refuse("missing frozen authority"))?;
    current.files.insert("sudo".into(), sudo);
    if current != intent.files {
        return Err(refuse("frozen source changed"));
    }
    if artifacts(source)? != intent.artifacts {
        return Err(refuse("source Artifacts changed"));
    }
    Ok(())
}

pub(super) fn artifacts(source: &Path) -> Result<Option<files::Inventory>> {
    let root = source.join(".issues");
    if root.try_exists()? {
        Ok(Some(files::inventory(&root)?))
    } else {
        Ok(None)
    }
}

impl Cutover {
    pub(super) fn guards(&self) -> Result<Vec<Permit>> {
        let source = &self.source;
        files::directory(&source.join(".concord"))?;
        let mut paths = vec![source.join(".concord.lock")];
        for entry in fs::read_dir(source.join(".concord"))? {
            let path = entry?.path();
            if path.extension().is_some_and(|ext| ext == "lock") {
                paths.push(path);
            }
        }
        paths.sort();
        let mut held = Vec::new();
        for path in paths {
            if path.try_exists()? {
                files::digest(&path)?;
            }
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(path)?;
            let gate =
                Gate::new(file, Policy::EXCLUSIVE).map_err(|error| refuse(error.to_string()))?;
            let permit = gate.acquire(Wait::immediate()).map_err(|_| {
                refuse("a legacy lock is occupied; stop writers and retry without replacing locks")
            })?;
            held.push(permit);
        }
        Ok(held)
    }

    pub(super) fn release(held: Vec<Permit>, outcome: Result<()>) -> Result<()> {
        let failures = held
            .into_iter()
            .filter_map(|permit| permit.release().err().map(|error| error.to_string()))
            .collect::<Vec<_>>();
        if failures.is_empty() {
            return outcome;
        }
        let result = match outcome {
            Ok(()) => serde_json::json!({"state":"completed"}),
            Err(error) => {
                serde_json::json!({"code":error.code(),"message":error.message(),"details":error.details()})
            }
        };
        Err(Error::detailed(
            "concord.cutover.release",
            "cutover lock release failed; preserve the outcome and stop",
            serde_json::json!({"outcome":result,"release":failures}),
        ))
    }
}
