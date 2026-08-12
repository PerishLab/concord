use crate::{Error, Result};
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

pub struct Managed<'a> {
    path: &'a Path,
}

pub fn at(path: &Path) -> Managed<'_> {
    Managed { path }
}

impl Managed<'_> {
    pub fn directory(&self) -> Result<()> {
        std::fs::create_dir_all(self.path)?;
        self.mode(0o700)
    }

    pub fn file(&self, text: &str) -> Result<()> {
        self.write(text.as_bytes(), 0o600)
    }

    pub fn write(&self, bytes: &[u8], permissions: u32) -> Result<()> {
        let parent = self
            .path
            .parent()
            .ok_or_else(|| Error::new("managed file has no parent"))?;
        at(parent).directory()?;
        let name = self
            .path
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| Error::new("managed filename is not utf8"))?;
        let temporary = parent.join(format!(".{name}.concord-{}", std::process::id()));
        let result = install(&temporary, self.path, bytes, permissions);
        if result.is_err() {
            let _ = std::fs::remove_file(&temporary);
        }
        result
    }

    pub fn copy(&self, source: &Path, permissions: u32) -> Result<()> {
        let parent = self
            .path
            .parent()
            .ok_or_else(|| Error::new("managed file has no parent"))?;
        at(parent).directory()?;
        let name = self
            .path
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| Error::new("managed filename is not utf8"))?;
        let temporary = parent.join(format!(".{name}.concord-{}", std::process::id()));
        let result = copy(source, &temporary, self.path, permissions);
        if result.is_err() {
            let _ = std::fs::remove_file(&temporary);
        }
        result
    }

    pub fn mode(&self, wanted: u32) -> Result<()> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(self.path, std::fs::Permissions::from_mode(wanted))?;
        }
        #[cfg(not(unix))]
        let _ = (self.path, wanted);
        Ok(())
    }

    pub fn held(&self) -> Result<Option<u32>> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            Ok(Some(
                std::fs::metadata(self.path)?.permissions().mode() & 0o777,
            ))
        }
        #[cfg(not(unix))]
        {
            let _ = self.path;
            Ok(None)
        }
    }
}

fn install(temporary: &Path, path: &Path, bytes: &[u8], permissions: u32) -> Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    at(temporary).mode(permissions)?;
    std::fs::rename(temporary, path)?;
    Ok(())
}

fn copy(source: &Path, temporary: &Path, target: &Path, permissions: u32) -> Result<()> {
    let mut source = std::fs::File::open(source)?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(temporary)?;
    std::io::copy(&mut source, &mut file)?;
    file.sync_all()?;
    at(temporary).mode(permissions)?;
    std::fs::rename(temporary, target)?;
    Ok(())
}
