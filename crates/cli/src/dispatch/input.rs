use concord_core::{Error, Result};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};

pub(super) struct Input {
    content: String,
    source: Source,
    limit: usize,
}

enum Source {
    Stdin,
    File {
        requested: PathBuf,
        resolved: PathBuf,
        digest: String,
    },
}

impl Input {
    #[locus::trace(with = concord_core::observation::view())]
    pub fn load(path: &Path, limit: usize, managed_root: &Path) -> Result<Self> {
        if is_stdin(path) {
            let bytes = bounded(std::io::stdin().lock(), limit, "stdin")?;
            return Ok(Self {
                content: utf8(bytes, "stdin")?,
                source: Source::Stdin,
                limit,
            });
        }
        let metadata = std::fs::symlink_metadata(path).map_err(|error| {
            Error::typed(
                "memory.input_read",
                format!("cannot inspect input {}: {error}", path.display()),
            )
        })?;
        if metadata.file_type().is_symlink() {
            return Err(Error::typed(
                "memory.input_symlink",
                format!("memory input refuses symbolic link {}", path.display()),
            ));
        }
        if !metadata.is_file() {
            return Err(Error::typed(
                "memory.input_type",
                format!("memory input is not a regular file: {}", path.display()),
            ));
        }
        let resolved = path.canonicalize().map_err(|error| {
            Error::typed(
                "memory.input_read",
                format!("cannot resolve input {}: {error}", path.display()),
            )
        })?;
        if managed_root.exists()
            && resolved.starts_with(managed_root.canonicalize().map_err(|error| {
                Error::typed(
                    "memory.input_read",
                    format!(
                        "cannot resolve managed memory root {}: {error}",
                        managed_root.display()
                    ),
                )
            })?)
        {
            return Err(Error::typed(
                "memory.input_managed",
                format!(
                    "memory input cannot be read from managed memory: {}",
                    resolved.display()
                ),
            ));
        }
        let bytes = bounded(
            std::fs::File::open(path).map_err(|error| {
                Error::typed(
                    "memory.input_read",
                    format!("cannot open input {}: {error}", path.display()),
                )
            })?,
            limit,
            &path.display().to_string(),
        )?;
        let digest = digest(&bytes);
        Ok(Self {
            content: utf8(bytes, &path.display().to_string())?,
            source: Source::File {
                requested: path.to_path_buf(),
                resolved,
                digest,
            },
            limit,
        })
    }

    pub fn content(&self) -> &str {
        &self.content
    }

    pub fn same_file(&self, other: &Self) -> bool {
        match (&self.source, &other.source) {
            (
                Source::File { resolved: left, .. },
                Source::File {
                    resolved: right, ..
                },
            ) => left == right,
            _ => false,
        }
    }

    pub fn verify_cleanup(&self) -> Result<()> {
        let Source::File {
            requested,
            digest: held_digest,
            ..
        } = &self.source
        else {
            return Ok(());
        };
        let metadata = std::fs::symlink_metadata(requested).map_err(|error| {
            Error::typed(
                "memory.input_cleanup",
                format!(
                    "cannot re-inspect input {} after apply: {error}",
                    requested.display()
                ),
            )
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(Error::typed(
                "memory.input_cleanup",
                format!(
                    "input {} changed type after apply; cleanup refused",
                    requested.display()
                ),
            ));
        }
        let bytes = bounded(
            std::fs::File::open(requested).map_err(|error| {
                Error::typed(
                    "memory.input_cleanup",
                    format!(
                        "cannot reopen input {} after apply: {error}",
                        requested.display()
                    ),
                )
            })?,
            self.limit,
            &requested.display().to_string(),
        )?;
        if digest(&bytes) != *held_digest {
            return Err(Error::typed(
                "memory.input_cleanup",
                format!(
                    "input {} changed after apply; cleanup refused",
                    requested.display()
                ),
            ));
        }
        Ok(())
    }

    pub fn remove(&self) -> Result<()> {
        let Source::File { requested, .. } = &self.source else {
            return Ok(());
        };
        std::fs::remove_file(requested).map_err(|error| {
            Error::typed(
                "memory.input_cleanup",
                format!(
                    "cannot consume input {} after apply: {error}",
                    requested.display()
                ),
            )
        })
    }

    pub fn path(&self) -> Option<String> {
        match &self.source {
            Source::Stdin => None,
            Source::File { requested, .. } => Some(requested.display().to_string()),
        }
    }
}

pub(super) fn is_stdin(path: &Path) -> bool {
    path == Path::new("-")
}

fn bounded(mut reader: impl Read, limit: usize, label: &str) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader
        .by_ref()
        .take((limit + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| {
            Error::typed(
                "memory.input_read",
                format!("cannot read input {label}: {error}"),
            )
        })?;
    if bytes.len() > limit {
        return Err(Error::typed(
            "memory.input_limit",
            format!("memory input {label} exceeds {limit} bytes"),
        ));
    }
    Ok(bytes)
}

fn utf8(bytes: Vec<u8>, label: &str) -> Result<String> {
    String::from_utf8(bytes).map_err(|_| {
        Error::typed(
            "memory.input_utf8",
            format!("memory input {label} is not UTF-8"),
        )
    })
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
