use super::super::Evidence;
use crate::{Error, Result};
use sha2::{Digest, Sha256};
use std::path::Path;

pub(super) struct Trace<'a> {
    space: &'a Path,
    evidence: &'a mut Vec<Evidence>,
}

impl<'a> Trace<'a> {
    pub fn new(space: &'a Path, evidence: &'a mut Vec<Evidence>) -> Self {
        Self { space, evidence }
    }

    pub fn walk(&mut self, root: &Path, label: &str, links: bool) -> Result<()> {
        let metadata = std::fs::symlink_metadata(root)?;
        let kind = metadata.file_type();
        if kind.is_symlink() {
            if !links {
                return Err(foreign(root));
            }
            return self.one(root, &format!("{label}-link"));
        }
        if kind.is_file() {
            return self.one(root, &format!("{label}-file"));
        }
        if !kind.is_dir() {
            return Err(foreign(root));
        }
        self.one(root, &format!("{label}-directory"))?;
        for entry in std::fs::read_dir(root)? {
            self.walk(&entry?.path(), label, links)?;
        }
        Ok(())
    }

    pub fn one(&mut self, path: &Path, kind: &str) -> Result<()> {
        let metadata = std::fs::symlink_metadata(path)?;
        let held = metadata.file_type();
        let (bytes, digest) = if held.is_file() {
            let mut file = std::fs::File::open(path)?;
            let mut hash = Sha256::new();
            let bytes = std::io::copy(&mut file, &mut Hash(&mut hash))?;
            (bytes, Some(format!("{:x}", hash.finalize())))
        } else if held.is_symlink() {
            let target = std::fs::read_link(path)?;
            let bytes = target.as_os_str().as_encoded_bytes();
            (
                bytes.len() as u64,
                Some(format!("{:x}", Sha256::digest(bytes))),
            )
        } else if held.is_dir() {
            (0, None)
        } else {
            return Err(foreign(path));
        };
        self.evidence.push(Evidence {
            path: relative(self.space, path)?,
            kind: kind.to_string(),
            bytes,
            mode: mode(&metadata),
            digest,
        });
        Ok(())
    }
}

struct Hash<'a>(&'a mut Sha256);

impl std::io::Write for Hash<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn relative(space: &Path, path: &Path) -> Result<String> {
    path.strip_prefix(space)
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .map_err(|_| {
            Error::typed(
                "concord.migration.path",
                format!("evidence lies outside Space: {}", path.display()),
            )
        })
}

#[cfg(unix)]
fn mode(metadata: &std::fs::Metadata) -> Option<u32> {
    use std::os::unix::fs::PermissionsExt;
    Some(metadata.permissions().mode() & 0o777)
}

#[cfg(not(unix))]
fn mode(_: &std::fs::Metadata) -> Option<u32> {
    None
}

pub(super) fn foreign(path: &Path) -> Error {
    Error::typed(
        "concord.migration.territory",
        format!(
            "migration cannot model filesystem entry: {}",
            path.display()
        ),
    )
}
