use super::{Intent, Ready, Result, Seat, files, git, journal, refuse};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub(super) fn absent(path: &Path) -> Result<()> {
    match path.symlink_metadata() {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
        Ok(_) => Err(refuse(format!("expected absence: {}", path.display()))),
    }
}

pub(super) fn supported(inventory: &files::Inventory) -> Result<()> {
    for path in inventory.files.keys() {
        let text = path.to_string_lossy();
        let observation = text.starts_with("activity/") || text.starts_with("occupancy/");
        let name = text.starts_with("integration-") || text.starts_with("land-");
        let suffix = text.ends_with(".lock") || text.ends_with(".json");
        let lock = !text.contains('/') && name && suffix;
        let required = text == "sudo" || text == "estate.sqlite3";
        if !required && !observation && !lock {
            return Err(refuse(
                "unknown controls/SQLite sidecars; checkpoint through the old operator",
            ));
        }
    }
    for name in ["sudo", "estate.sqlite3"] {
        if !inventory.files.contains_key(Path::new(name)) {
            return Err(refuse("missing source control"));
        }
    }
    Ok(())
}

pub(super) fn inventory(root: &Path) -> Result<files::Inventory> {
    files::directory(root)?;
    let mut result = files::Inventory {
        files: BTreeMap::new(),
        directories: Vec::new(),
    };
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        let name = entry.file_name();
        if name == "issues" || name == "sudo" {
            continue;
        }
        if name == "locks" {
            result.directories.push(PathBuf::from("locks"));
            let part = files::inventory(&entry.path())?;
            result.files.extend(
                part.files
                    .into_iter()
                    .map(|(path, hash)| (PathBuf::from("locks").join(path), hash)),
            );
            result.directories.extend(
                part.directories
                    .into_iter()
                    .map(|path| PathBuf::from("locks").join(path)),
            );
        } else if name == "estate.sqlite3" || name == "identity.json" {
            result
                .files
                .insert(PathBuf::from(name), files::digest(&entry.path())?);
        } else {
            return Err(refuse("unknown target control entry"));
        }
    }
    result.directories.sort();
    Ok(result)
}

pub(super) fn exact(home: &Path, ready: &Ready) -> Result<()> {
    if inventory(&home.join("workspace"))? != ready.files
        || git::Git::new(home)?.tool()? != ready.git
    {
        return Err(refuse("target controls or Git binding changed"));
    }
    Ok(())
}

pub(super) async fn validate(home: &Path, ready: &Ready) -> Result<()> {
    exact(home, ready)?;
    let estate = Seat::new(home).open().await?;
    if estate.identity() != ready.space || !estate.structure().await?.agrees() {
        return Err(refuse("target identity/model changed"));
    }
    if estate.worktrees().await?.len() != ready.members.len() {
        return Err(refuse("target Member catalogue changed"));
    }
    Ok(())
}

pub(super) fn authority(intent: &Intent, path: &Path) -> Result<()> {
    let expected = intent
        .files
        .files
        .get(Path::new("sudo"))
        .ok_or_else(|| refuse("missing original possession fingerprint"))?;
    if files::digest(path)? != *expected {
        return Err(refuse("possession fingerprint changed"));
    }
    Ok(())
}

pub(super) fn discard(intent: &Intent) -> Result<()> {
    let staged = journal::stage(&intent.home);
    if !staged.try_exists()? {
        return Ok(());
    }
    let inventory = files::inventory(&staged)?;
    for path in inventory.files.keys() {
        if !matches!(
            path.to_str(),
            Some(
                "workspace/estate.sqlite3"
                    | "workspace/sudo"
                    | "workspace/identity.json"
                    | "workspace/locks/estate.lock"
            )
        ) {
            return Err(refuse("unknown staged payload refuses abort"));
        }
    }
    let ready = journal::root(&intent.home).join("ready.json");
    if ready.try_exists()? {
        exact(&staged, &super::Ready::read(&intent.home)?)?;
    }
    files::erase(&staged, &inventory)
}

pub(super) fn cleanup(home: &Path) -> Result<()> {
    let root = journal::root(home);
    for name in [
        "intent.pending",
        "ready.pending",
        "ready.json",
        "intent.json",
    ] {
        let path = root.join(name);
        if path.try_exists()? {
            files::digest(&path)?;
            std::fs::remove_file(path)?;
        }
    }
    std::fs::remove_dir(root)?;
    Ok(())
}
