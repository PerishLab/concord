mod integrity;

use super::{Audit, permission};
use crate::Result;
use crate::path::at;
use std::path::Path;

pub(super) fn permissions(audit: &mut Audit, root: &Path) -> Result<()> {
    if !root.exists() {
        return Ok(());
    }
    permission(audit, root, 0o700)?;
    if !root.join("MAIN.md").is_file() {
        audit.fault(
            "presence",
            &root.join("MAIN.md"),
            "task memory exists without MAIN.md",
        );
    }
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            audit.fault("memory", &path, "symbolic links are not managed memory");
        } else if entry.file_name() == "resources" && kind.is_dir() {
            permission(audit, &path, 0o700)?;
            visit_resources(audit, &path)?;
        } else if kind.is_dir() {
            permission(audit, &path, 0o700)?;
            visit(audit, &path)?;
        } else {
            permission(audit, &path, 0o600)?;
        }
    }
    integrity::inspect(audit, root)?;
    Ok(())
}

fn visit(audit: &mut Audit, root: &Path) -> Result<()> {
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            audit.fault("memory", &path, "symbolic links are not managed memory");
        } else if kind.is_dir() {
            permission(audit, &path, 0o700)?;
            visit(audit, &path)?;
        } else {
            permission(audit, &path, 0o600)?;
        }
    }
    Ok(())
}

fn visit_resources(audit: &mut Audit, root: &Path) -> Result<()> {
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            audit.fault(
                "resource",
                &path,
                "symbolic links are not private resources",
            );
        } else if kind.is_dir() {
            permission(audit, &path, 0o700)?;
            visit_resources(audit, &path)?;
        } else if let Some(held) = at(&path).held()?
            && held & 0o077 != 0
        {
            audit.fault(
                "permission",
                &path,
                format!("mode {held:04o} exposes a private resource"),
            );
        }
    }
    Ok(())
}
