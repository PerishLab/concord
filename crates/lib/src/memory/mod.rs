pub(crate) mod copy;
pub(crate) mod format;
mod phase;

use crate::{Error, Legacy, Result};
use std::path::{Path, PathBuf};

pub(crate) struct Memory<'a> {
    task: &'a Legacy,
}

pub(crate) struct Read {
    pub path: String,
    pub content: String,
}

impl<'a> Memory<'a> {
    pub(crate) fn new(task: &'a Legacy) -> Self {
        Self { task }
    }

    #[locus::trace(with = crate::observation::view())]
    pub(crate) fn read(&self) -> Result<Read> {
        read(&self.main())
    }

    pub(crate) fn root(&self) -> PathBuf {
        self.task.path().join(".task")
    }

    fn main(&self) -> PathBuf {
        self.root().join("MAIN.md")
    }
}

fn read(path: &Path) -> Result<Read> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| Error::new(format!("cannot read memory {}: {error}", path.display())))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(Error::typed(
            "memory.read_type",
            format!("memory is not a regular file: {}", path.display()),
        ));
    }
    if metadata.len() > format::RAW as u64 {
        return Err(Error::typed(
            "memory.read_limit",
            format!(
                "memory {} is {} bytes; raw read maximum is {}",
                path.display(),
                metadata.len(),
                format::RAW
            ),
        ));
    }
    let bytes = std::fs::read(path)
        .map_err(|error| Error::new(format!("cannot read memory {}: {error}", path.display())))?;
    let content = String::from_utf8(bytes)
        .map_err(|_| Error::new(format!("memory is not utf8: {}", path.display())))?;
    Ok(Read {
        path: path.display().to_string(),
        content,
    })
}
