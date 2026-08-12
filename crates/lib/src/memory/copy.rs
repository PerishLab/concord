use crate::path::at;
use crate::{Error, Result};
use std::path::Path;

pub(crate) fn tree(source: &Path, target: &Path) -> Result<()> {
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let to = target.join(entry.file_name());
        if kind.is_symlink() {
            return Err(Error::new(format!(
                "resource import refuses symbolic link {}",
                entry.path().display()
            )));
        }
        if kind.is_dir() {
            at(&to).directory()?;
            tree(&entry.path(), &to)?;
        } else if kind.is_file() {
            file(&entry.path(), &to)?;
        } else {
            return Err(Error::new(format!(
                "resource import refuses special file {}",
                entry.path().display()
            )));
        }
    }
    Ok(())
}

pub(crate) fn file(source: &Path, target: &Path) -> Result<()> {
    at(target).copy(source, if executable(source)? { 0o700 } else { 0o600 })
}

fn executable(path: &Path) -> Result<bool> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        Ok(std::fs::metadata(path)?.permissions().mode() & 0o100 != 0)
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Ok(false)
    }
}
