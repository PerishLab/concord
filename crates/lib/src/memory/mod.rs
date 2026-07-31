mod copy;
pub(crate) mod format;
mod phase;
mod resource;

use crate::path::{at, revision};
use crate::{Error, Result, TaskRef};
use serde::Serialize;
use std::path::{Path, PathBuf};

pub use format::{
    MAX_MAIN_BYTES, MAX_MAIN_LINES, MAX_PHASE_BYTES, MAX_PHASE_LINES, MAX_RAW_READ_BYTES,
};
pub use phase::PhaseEntry;

pub struct Memory<'a> {
    task: &'a TaskRef,
}

#[derive(Clone, Debug, Serialize)]
pub struct MemoryRead {
    pub path: String,
    pub revision: String,
    pub content: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct MemoryChange {
    pub path: String,
    pub revision: String,
    pub changed: bool,
}

impl<'a> Memory<'a> {
    pub fn new(task: &'a TaskRef) -> Self {
        Self { task }
    }

    pub fn init(&self, content: &str) -> Result<()> {
        format::validate_main(content)?;
        let _lock = self.task.lock()?;
        self.task.ensure_exact()?;
        let root = self.root();
        if root.exists() {
            return Err(Error::new(format!(
                "task memory already exists: {}",
                root.display()
            )));
        }
        let result = (|| {
            at(&root).directory()?;
            at(&self.main()).file(content)
        })();
        if result.is_err() {
            let _ = std::fs::remove_dir(&root);
        }
        result
    }

    #[locus::trace(with = crate::observation::view())]
    pub fn read(&self) -> Result<MemoryRead> {
        read(&self.main())
    }

    pub fn read_sections(&self, keys: &[String]) -> Result<MemoryRead> {
        let mut held = self.read()?;
        held.content = format::project(&held.content, &held.revision, keys)?;
        Ok(held)
    }

    pub fn write(&self, expected: &str, content: &str) -> Result<MemoryChange> {
        format::validate_main(content)?;
        let _lock = self.task.lock()?;
        self.task.ensure_exact()?;
        self.write_held(expected, content)
    }

    fn write_held(&self, expected: &str, content: &str) -> Result<MemoryChange> {
        let current = self.read()?;
        if current.revision != expected {
            return Err(revision_error(expected, &current.revision));
        }
        let held_kind = format::main_kind(&current.content)?;
        let next_kind = format::main_kind(content)?;
        if held_kind == format::Kind::V1 && next_kind != format::Kind::V1 {
            return Err(Error::typed(
                "memory.downgrade",
                "ordinary memory write cannot downgrade concord-memory:v1",
            ));
        }
        if held_kind == format::Kind::Legacy && next_kind == format::Kind::V1 {
            self.ensure_structured_transition()?;
        }
        if current.content == content {
            return Ok(MemoryChange {
                path: current.path,
                revision: current.revision,
                changed: false,
            });
        }
        at(&self.main()).file(content)?;
        Ok(change(self.read()?, true))
    }

    #[locus::trace(with = crate::observation::view())]
    pub fn patch(&self, expected: Option<&str>, content: &str) -> Result<MemoryChange> {
        let patch = format::parse_patch(content)?;
        if let Some(expected) = expected
            && expected != patch.revision
        {
            return Err(Error::typed(
                "memory.patch_expect",
                format!(
                    "--expect {expected} differs from patch revision {}",
                    patch.revision
                ),
            ));
        }
        let _lock = self.task.lock()?;
        self.task.ensure_exact()?;
        let held = self.read()?;
        if held.revision != patch.revision {
            return Err(revision_error(&patch.revision, &held.revision));
        }
        let updated = format::apply_patch(&held.content, content, &patch)?;
        if updated == held.content {
            return Ok(change(held, false));
        }
        at(&self.main()).file(&updated)?;
        Ok(change(self.read()?, true))
    }

    pub fn links(&self) -> Result<Vec<PathBuf>> {
        let root = self.root();
        if !root.exists() {
            return Ok(Vec::new());
        }
        let mut found = Vec::new();
        links(&root, &mut found)?;
        found.sort();
        Ok(found)
    }

    pub fn normalize(&self) -> Result<()> {
        let root = self.root();
        if !root.exists() {
            return Ok(());
        }
        normalize(&root, false)
    }

    pub fn root(&self) -> PathBuf {
        self.task.path().join(".task")
    }

    pub fn main(&self) -> PathBuf {
        self.root().join("MAIN.md")
    }
}

fn read(path: &Path) -> Result<MemoryRead> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| Error::new(format!("cannot read memory {}: {error}", path.display())))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(Error::typed(
            "memory.read_type",
            format!("memory is not a regular file: {}", path.display()),
        ));
    }
    if metadata.len() > MAX_RAW_READ_BYTES as u64 {
        return Err(Error::typed(
            "memory.read_limit",
            format!(
                "memory {} is {} bytes; raw read maximum is {}",
                path.display(),
                metadata.len(),
                MAX_RAW_READ_BYTES
            ),
        ));
    }
    let bytes = std::fs::read(path)
        .map_err(|error| Error::new(format!("cannot read memory {}: {error}", path.display())))?;
    let content = String::from_utf8(bytes.clone())
        .map_err(|_| Error::new(format!("memory is not utf8: {}", path.display())))?;
    Ok(MemoryRead {
        path: path.display().to_string(),
        revision: revision(&bytes),
        content,
    })
}

fn change(read: MemoryRead, changed: bool) -> MemoryChange {
    MemoryChange {
        path: read.path,
        revision: read.revision,
        changed,
    }
}

fn revision_error(expected: &str, found: &str) -> Error {
    Error::typed(
        "memory.revision_changed",
        format!("memory revision changed: expected {expected}, found {found}"),
    )
    .with_details(serde_json::json!({
        "expected_revision": expected,
        "current_revision": found,
    }))
}

fn links(root: &Path, found: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            found.push(entry.path());
        } else if kind.is_dir() {
            links(&entry.path(), found)?;
        }
    }
    Ok(())
}

fn normalize(root: &Path, resources: bool) -> Result<()> {
    at(root).mode(0o700)?;
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            continue;
        }
        if kind.is_dir() {
            normalize(&path, resources || entry.file_name() == "resources")?;
        } else {
            let executable = resources && held_owner_execute(&path)?;
            at(&path).mode(if executable { 0o700 } else { 0o600 })?;
        }
    }
    Ok(())
}

fn held_owner_execute(path: &Path) -> Result<bool> {
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
