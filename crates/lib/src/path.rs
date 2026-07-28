use crate::{Error, Result};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::{Path, PathBuf};

pub fn expand(value: &str, base: &Path) -> Result<PathBuf> {
    let path = if value == "~" || value.starts_with("~/") {
        let home = crate::config::home()?;
        let suffix = value.strip_prefix("~/").unwrap_or("");
        home.join(suffix)
    } else {
        PathBuf::from(value)
    };
    let path = if path.is_absolute() {
        path
    } else {
        base.join(path)
    };
    path.canonicalize().map_err(|error| {
        Error::new(format!(
            "cannot resolve repository source {}: {error}",
            path.display()
        ))
    })
}

pub fn private_dir(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path)?;
    mode(path, 0o700)
}

pub fn private_file(path: &Path, text: &str) -> Result<()> {
    replace(path, text.as_bytes(), 0o600)
}

pub fn replace(path: &Path, bytes: &[u8], permissions: u32) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| Error::new("managed file has no parent"))?;
    private_dir(parent)?;
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| Error::new("managed filename is not utf8"))?;
    let temporary = parent.join(format!(".{name}.concord-{}", std::process::id()));
    let result = install(&temporary, path, bytes, permissions);
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

fn install(temporary: &Path, path: &Path, bytes: &[u8], permissions: u32) -> Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    mode(temporary, permissions)?;
    std::fs::rename(temporary, path)?;
    Ok(())
}

pub fn revision(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn mode(path: &Path, wanted: u32) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(wanted))?;
    }
    #[cfg(not(unix))]
    let _ = (path, wanted);
    Ok(())
}

pub fn held_mode(path: &Path) -> Result<Option<u32>> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        Ok(Some(std::fs::metadata(path)?.permissions().mode() & 0o777))
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Ok(None)
    }
}
