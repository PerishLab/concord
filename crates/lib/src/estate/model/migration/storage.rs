use super::{Evidence, Receipt, SOURCE, TARGET};
use crate::path::at;
use crate::{Error, Result};
use sha2::{Digest as _, Sha256};
use std::path::Path;

pub(super) fn regular(path: &Path) -> Result<()> {
    let metadata = std::fs::symlink_metadata(path).map_err(|error| {
        Error::typed(
            "concord.migration.path",
            format!("cannot inspect migration path {}: {error}", path.display()),
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() && !metadata.is_dir() {
        return Err(Error::typed(
            "concord.migration.path",
            format!(
                "migration path is not a real file or directory: {}",
                path.display()
            ),
        ));
    }
    Ok(())
}

pub(super) fn settled(database: &Path) -> Result<()> {
    for suffix in ["-journal", "-wal", "-shm"] {
        let sidecar = Path::new(&format!("{}{suffix}", database.display())).to_path_buf();
        if sidecar.exists() {
            return Err(Error::typed(
                "concord.migration.database",
                format!(
                    "database has an unsettled SQLite sidecar: {}",
                    sidecar.display()
                ),
            ));
        }
    }
    Ok(())
}

pub(super) fn evidence(path: &Path) -> Result<Evidence> {
    regular(path)?;
    if !path.is_file() {
        return Err(Error::typed(
            "concord.migration.path",
            format!("migration evidence is not a file: {}", path.display()),
        ));
    }
    let bytes = std::fs::read(path)?;
    Ok(Evidence {
        path: path.to_path_buf(),
        bytes: bytes.len() as u64,
        digest: format!("{:x}", Sha256::digest(bytes)),
    })
}

pub(super) fn copy(source: &Path, target: &Path) -> Result<Evidence> {
    if target.exists() {
        return Err(Error::typed(
            "concord.migration.target",
            format!("migration target already exists: {}", target.display()),
        ));
    }
    at(target).copy(source, 0o600)?;
    evidence(target)
}

pub(super) fn activate(stage: &Path, live: &Path) -> Result<()> {
    regular(stage)?;
    std::fs::rename(stage, live).map_err(|error| {
        Error::typed(
            "concord.migration.activate",
            format!(
                "cannot atomically activate {} as {}: {error}",
                stage.display(),
                live.display()
            ),
        )
    })
}

pub(super) fn restore(backup: &Path, live: &Path) -> Result<()> {
    let parent = live
        .parent()
        .ok_or_else(|| Error::typed("concord.migration.path", "estate has no parent"))?;
    let temporary = parent.join(".estate.sqlite3.rollback");
    if temporary.exists() {
        return Err(Error::typed(
            "concord.migration.rollback",
            format!(
                "rollback temporary path already exists: {}",
                temporary.display()
            ),
        ));
    }
    at(&temporary).copy(backup, 0o600)?;
    std::fs::rename(&temporary, live).map_err(|error| {
        Error::typed(
            "concord.migration.rollback",
            format!("cannot atomically restore estate: {error}"),
        )
    })
}

pub(super) fn record(receipt: &Receipt) -> Result<()> {
    let envelope = serde_json::json!({"version": 1, "receipt": receipt});
    let bytes = serde_json::to_vec_pretty(&envelope)
        .map_err(|error| Error::typed("concord.migration.receipt", error.to_string()))?;
    at(&receipt.record).write(&bytes, 0o600)
}

pub(super) fn fingerprint(database: &Evidence, sudo: &Evidence) -> String {
    let claim = format!(
        "{SOURCE}\n{TARGET}\n{}\n{}\n{}\n{}\n",
        database.bytes, database.digest, sudo.bytes, sudo.digest
    );
    format!("{:x}", Sha256::digest(claim.as_bytes()))
}
