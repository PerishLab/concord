use crate::path::private_dir;
use crate::{Error, Result};
use std::path::Path;

pub(super) fn tree(source: &Path, target: &Path) -> Result<()> {
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
            private_dir(&to)?;
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

pub(super) fn file(source: &Path, target: &Path) -> Result<()> {
    crate::path::copy_private(
        source,
        target,
        if super::held_owner_execute(source)? {
            0o700
        } else {
            0o600
        },
    )
}
