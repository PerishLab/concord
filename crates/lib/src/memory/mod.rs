mod copy;

use crate::path::{mode, private_dir, private_file, revision};
use crate::{Error, ImportPreflight, Result, TaskRef};
use serde::Serialize;
use std::path::{Path, PathBuf};

pub struct Memory<'a> {
    task: &'a TaskRef,
}

#[derive(Clone, Debug, Serialize)]
pub struct MemoryRead {
    pub path: String,
    pub revision: String,
    pub content: String,
}

impl<'a> Memory<'a> {
    pub fn new(task: &'a TaskRef) -> Self {
        Self { task }
    }

    pub fn init(&self, content: &str) -> Result<()> {
        let _lock = self.task.lock()?;
        self.task.ensure_exact()?;
        let root = self.root();
        if root.exists() {
            return Err(Error::new(format!(
                "task memory already exists: {}",
                root.display()
            )));
        }
        private_dir(&root)?;
        private_file(&self.main(), content)
    }

    pub fn read(&self) -> Result<MemoryRead> {
        read(&self.main())
    }

    pub fn write(&self, expected: &str, content: &str) -> Result<MemoryRead> {
        let _lock = self.task.lock()?;
        self.task.ensure_exact()?;
        self.write_held(expected, content)
    }

    fn write_held(&self, expected: &str, content: &str) -> Result<MemoryRead> {
        let current = self.read()?;
        if current.revision != expected {
            return Err(Error::new(format!(
                "memory revision changed: expected {expected}, found {}",
                current.revision
            )));
        }
        private_file(&self.main(), content)?;
        self.read()
    }

    pub fn settle(
        &self,
        expected: &str,
        phase: &str,
        current: &str,
    ) -> Result<(MemoryRead, PathBuf)> {
        let _lock = self.task.lock()?;
        self.task.ensure_exact()?;
        let held = self.read()?;
        if held.revision != expected {
            return Err(Error::new(format!(
                "memory revision changed: expected {expected}, found {}",
                held.revision
            )));
        }
        let phases = self.root().join("phases");
        private_dir(&phases)?;
        let path = phases.join(self.next_phase(&phases)?);
        private_file(&path, phase)?;
        match self.write_held(expected, current) {
            Ok(read) => Ok((read, path)),
            Err(error) => Err(Error::new(format!(
                "{error}; complete phase remains at {}",
                path.display()
            ))),
        }
    }

    pub fn allocate(&self, name: &str) -> Result<PathBuf> {
        crate::model::component("resource seat", name)?;
        let _lock = self.task.lock()?;
        self.task.ensure_exact()?;
        self.require_root()?;
        let root = self.root().join("resources");
        private_dir(&root)?;
        let seat = root.join(name);
        if seat.exists() {
            return Err(Error::new(format!(
                "resource seat already exists: {}",
                seat.display()
            )));
        }
        private_dir(&seat)?;
        Ok(seat)
    }

    pub fn import(&self, name: &str, source: &Path) -> Result<PathBuf> {
        crate::model::component("resource seat", name)?;
        let _lock = self.task.lock()?;
        self.task.ensure_exact()?;
        self.require_root()?;
        let root = self.root().join("resources");
        let seat = root.join(name);
        if seat.exists() {
            return Err(Error::new(format!(
                "resource seat already exists: {}",
                seat.display()
            )));
        }
        let preflight = crate::audit::resource::import_preflight(source, &seat, &self.root())?;
        let source = PathBuf::from(preflight.source);
        private_dir(&root)?;
        private_dir(&seat)?;
        let result = if source.is_dir() {
            copy::tree(&source, &seat)
        } else {
            let filename = source
                .file_name()
                .ok_or_else(|| Error::new("resource source has no filename"))?;
            copy::file(&source, &seat.join(filename))
        };
        if let Err(error) = result {
            let _ = std::fs::remove_dir_all(&seat);
            return Err(error);
        }
        Ok(seat)
    }

    pub fn preflight_import(&self, name: &str, source: &Path) -> Result<ImportPreflight> {
        crate::model::component("resource seat", name)?;
        self.task.ensure_exact()?;
        self.require_root()?;
        let seat = self.root().join("resources").join(name);
        if seat.exists() {
            return Err(Error::new(format!(
                "resource seat already exists: {}",
                seat.display()
            )));
        }
        crate::audit::resource::import_preflight(source, &seat, &self.root())
    }

    pub fn resources(&self) -> Result<Vec<PathBuf>> {
        let root = self.root().join("resources");
        if !root.exists() {
            return Ok(Vec::new());
        }
        let mut seats = std::fs::read_dir(root)?
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
            .map(|entry| entry.path())
            .collect::<Vec<_>>();
        seats.sort();
        Ok(seats)
    }

    pub fn resource(&self, name: &str) -> Result<PathBuf> {
        crate::model::component("resource seat", name)?;
        let seat = self.root().join("resources").join(name);
        if !seat.is_dir() {
            return Err(Error::new(format!(
                "resource seat does not exist: {}",
                seat.display()
            )));
        }
        Ok(seat)
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

    fn next_phase(&self, root: &Path) -> Result<String> {
        let mut highest = None;
        for entry in std::fs::read_dir(root)? {
            let entry = entry?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let number = name
                .strip_prefix("PHASE-")
                .and_then(|value| value.strip_suffix(".md"))
                .and_then(|value| value.parse::<u32>().ok());
            if let Some(number) = number {
                highest = Some(highest.map_or(number, |held: u32| held.max(number)));
            }
        }
        Ok(format!(
            "PHASE-{:02}.md",
            highest.map_or(0, |value| value + 1)
        ))
    }

    fn require_root(&self) -> Result<()> {
        if self.main().is_file() {
            Ok(())
        } else {
            Err(Error::new("resource seats require initialized task memory"))
        }
    }
}

fn read(path: &Path) -> Result<MemoryRead> {
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

fn normalize(root: &Path, resources: bool) -> Result<()> {
    mode(root, 0o700)?;
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            return Err(Error::new(format!(
                "managed memory contains a symbolic link: {}",
                path.display()
            )));
        }
        if kind.is_dir() {
            normalize(&path, resources || entry.file_name() == "resources")?;
        } else {
            let executable = resources && held_owner_execute(&path)?;
            mode(&path, if executable { 0o700 } else { 0o600 })?;
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
