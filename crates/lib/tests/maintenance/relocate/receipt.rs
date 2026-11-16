use super::{Intent, Relocate, Result, Seat, control, files, journal, refuse};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    schema: String,
    source: PathBuf,
    home: PathBuf,
    space: String,
}

pub(super) async fn completed(task: &Relocate) -> Result<bool> {
    let path = journal::root(&task.home).join("complete.json");
    if !path.try_exists()? {
        return Ok(false);
    }
    let value: Receipt = journal::read(&path)?;
    if value.schema != "concord.member-relocation/complete-v1"
        || value.source != task.source
        || value.home != task.home
    {
        return Err(refuse("completion receipt does not bind this source/home"));
    }
    for name in [".concord", ".issues", ".concord.lock"] {
        control::absent(&task.source.join(name))?;
    }
    let estate = Seat::new(&task.home).open().await?;
    if estate.identity() != value.space || !estate.structure().await?.agrees() {
        return Err(refuse("completed target identity/model changed"));
    }
    cleanup(&task.home)?;
    Ok(true)
}

pub(super) fn erase(intent: &Intent) -> Result<()> {
    let controls = intent.source.join(".concord");
    if controls.try_exists()? {
        control::absent(&controls.join("sudo"))?;
        let current = files::inventory(&controls)?;
        let mut expected = intent.files.clone();
        let sudo = expected
            .files
            .remove(Path::new("sudo"))
            .ok_or_else(|| refuse("missing source fingerprint"))?;
        expected.files.insert("sudo.offline".into(), sudo);
        if current
            .files
            .iter()
            .any(|(path, hash)| expected.files.get(path) != Some(hash))
            || current
                .directories
                .iter()
                .any(|path| !expected.directories.contains(path))
        {
            return Err(refuse("cleanup refuses changed or unknown old controls"));
        }
        files::erase(&controls, &expected)?;
    }
    let lock = intent.source.join(".concord.lock");
    if lock.try_exists()? {
        files::digest(&lock)?;
        fs::remove_file(lock)?;
    }
    Ok(())
}

pub(super) fn publish(task: &Relocate, space: &str) -> Result<()> {
    let path = journal::root(&task.home).join("complete.json");
    let pending = path.with_extension("pending");
    if pending.try_exists()? {
        files::digest(&pending)?;
        fs::remove_file(pending)?;
    }
    journal::write(
        &path,
        &Receipt {
            schema: "concord.member-relocation/complete-v1".into(),
            source: task.source.clone(),
            home: task.home.clone(),
            space: space.into(),
        },
    )
}

pub(super) fn cleanup(home: &Path) -> Result<()> {
    for name in ["intent.json", "ready.json"] {
        let path = journal::root(home).join(name);
        if path.try_exists()? {
            files::digest(&path)?;
            fs::remove_file(path)?;
        }
    }
    Ok(())
}
