use super::super::super::super::Agreement;
use super::{Artifact, Entry, Evidence, Inventory};
use crate::{Error, Result};
use sha2::{Digest as _, Sha256};
use std::path::{Path, PathBuf};

pub(super) fn artifact(task: &str, name: &str, path: &Path) -> Result<Artifact> {
    directory(path, "Artifact")?;
    let mut entries = Vec::new();
    walk(path, path, &mut entries)?;
    entries.sort_by_key(|entry| entry.path.clone());
    let bytes = entries.iter().map(|entry| entry.bytes).sum();
    let encoded = serde_json::to_vec(&entries)
        .map_err(|error| Error::typed("concord.transition.artifact", error.to_string()))?;
    Ok(Artifact {
        task: task.to_string(),
        name: name.to_string(),
        path: path.canonicalize()?,
        bytes,
        digest: format!("{:x}", Sha256::digest(encoded)),
        entries,
    })
}

fn walk(root: &Path, directory: &Path, entries: &mut Vec<Entry>) -> Result<()> {
    let mut paths = std::fs::read_dir(directory)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    paths.sort();
    for path in paths {
        let metadata = std::fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            return Err(territory(
                &path,
                "Artifact payload contains a symbolic link",
            ));
        }
        let relative = relative(root, &path)?;
        if metadata.is_dir() {
            entries.push(Entry {
                path: relative,
                kind: "directory".to_string(),
                bytes: 0,
                digest: None,
            });
            walk(root, &path, entries)?;
        } else if metadata.is_file() {
            let body = std::fs::read(&path)?;
            entries.push(Entry {
                path: relative,
                kind: "file".to_string(),
                bytes: body.len() as u64,
                digest: Some(format!("{:x}", Sha256::digest(body))),
            });
        } else {
            return Err(territory(
                &path,
                "Artifact payload is not a regular file or directory",
            ));
        }
    }
    Ok(())
}

fn relative(root: &Path, path: &Path) -> Result<String> {
    let path = path
        .strip_prefix(root)
        .map_err(|_| territory(path, "Artifact entry escapes its seat"))?;
    let parts = path
        .components()
        .map(|part| {
            part.as_os_str().to_str().ok_or_else(|| {
                Error::typed("concord.transition.artifact", "Artifact path is not UTF-8")
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(parts.join("/"))
}

fn directory(path: &Path, label: &str) -> Result<()> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| territory(path, &format!("cannot inspect {label} seat: {error}")))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(territory(
            path,
            &format!("{label} seat is not a direct directory"),
        ));
    }
    Ok(())
}

pub(super) fn evidence(path: &Path) -> Result<Evidence> {
    let metadata = std::fs::symlink_metadata(path).map_err(|error| {
        Error::typed(
            "concord.transition.path",
            format!(
                "cannot inspect transition source {}: {error}",
                path.display()
            ),
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(Error::typed(
            "concord.transition.path",
            format!("transition source is not a direct file: {}", path.display()),
        ));
    }
    let body = std::fs::read(path)?;
    Ok(Evidence {
        path: path.to_path_buf(),
        bytes: body.len() as u64,
        digest: format!("{:x}", Sha256::digest(body)),
    })
}

pub(super) fn settled(database: &Path) -> Result<()> {
    for suffix in ["-journal", "-wal", "-shm"] {
        let sidecar = PathBuf::from(format!("{}{suffix}", database.display()));
        if sidecar.exists() {
            return Err(Error::typed(
                "concord.transition.database",
                format!(
                    "database has an unsettled SQLite sidecar: {}",
                    sidecar.display()
                ),
            ));
        }
    }
    Ok(())
}

pub(super) fn agree(agreement: &Agreement) -> Result<()> {
    if agreement.agrees() {
        return Ok(());
    }
    Err(Error::detailed(
        "concord.transition.agreement",
        "v0.13 transition source has agreement faults",
        serde_json::to_value(agreement).map_err(|error| Error::new(error.to_string()))?,
    ))
}

pub(super) fn fingerprint(inventory: &Inventory) -> Result<String> {
    let encoded = serde_json::to_vec(inventory)
        .map_err(|error| Error::typed("concord.transition.inventory", error.to_string()))?;
    Ok(format!("{:x}", Sha256::digest(encoded)))
}

pub(super) fn exact(label: &str, expected: &Evidence, found: &Evidence) -> Result<()> {
    if expected == found {
        return Ok(());
    }
    Err(Error::typed(
        "concord.transition.drift",
        format!("{label} changed while inventory was being read"),
    ))
}

fn territory(path: &Path, message: &str) -> Error {
    Error::typed(
        "concord.transition.artifact",
        format!("{message}: {}", path.display()),
    )
}
