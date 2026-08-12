use crate::path::{at, expand};
use crate::protocol::model::component;
use crate::{Error, Registry, Result, Root, Task};
use fs2::FileExt;
use std::fs::File;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Space {
    root: PathBuf,
}

#[derive(Clone, Debug)]
pub struct Domain {
    name: String,
    root: PathBuf,
}

#[derive(Clone, Debug)]
pub struct Legacy {
    domain: Domain,
    task: Task,
}

pub(crate) struct Lock {
    file: File,
}

impl Space {
    pub fn new(root: Root) -> Self {
        Self {
            root: root.path().to_path_buf(),
        }
    }

    pub fn path(&self) -> &Path {
        &self.root
    }

    pub fn domains(&self) -> Result<Vec<Domain>> {
        let mut found = Vec::new();
        for entry in std::fs::read_dir(&self.root)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            let domain = Domain::new(&self.root, &name)?;
            if domain.manifest().is_file() {
                found.push(domain);
            }
        }
        found.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(found)
    }

    pub fn domain(&self, name: &str) -> Result<Domain> {
        let domain = Domain::new(&self.root, name)?;
        if !domain.manifest().is_file() {
            return Err(Error::new(format!("unknown managed domain {name}")));
        }
        Ok(domain)
    }

    pub(crate) fn lock(&self) -> Result<Lock> {
        let path = self.root.join(".concord.lock");
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)?;
        at(&path).mode(0o600)?;
        file.lock_exclusive()?;
        Ok(Lock { file })
    }
}

impl Domain {
    pub fn new(space: &Path, name: &str) -> Result<Self> {
        component("domain name", name)?;
        Ok(Self {
            name: name.to_string(),
            root: space.join(name),
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn path(&self) -> &Path {
        &self.root
    }

    pub fn tasks(&self) -> PathBuf {
        self.root.join(".tasks")
    }

    pub fn manifest(&self) -> PathBuf {
        self.tasks().join("tasks.toml")
    }

    pub fn task(&self, name: &str) -> Result<Legacy> {
        component("task name", name)?;
        let task = self
            .read()?
            .task
            .into_iter()
            .find(|task| task.name == name)
            .ok_or_else(|| Error::new(format!("task not found: {}/{}", self.name, name)))?;
        Ok(Legacy {
            domain: self.clone(),
            task,
        })
    }

    pub fn registry(&self) -> Result<Registry> {
        self.read()
    }

    fn read(&self) -> Result<Registry> {
        let path = self.manifest();
        let raw = std::fs::read(&path).map_err(|error| {
            Error::new(format!("cannot read registry {}: {error}", path.display()))
        })?;
        let text = std::str::from_utf8(&raw)
            .map_err(|_| Error::new(format!("registry is not utf8: {}", path.display())))?;
        let registry: Registry = toml::from_str(text)?;
        registry.validate()?;
        Ok(registry)
    }
}

impl Legacy {
    pub fn domain(&self) -> &Domain {
        &self.domain
    }

    pub fn task(&self) -> &Task {
        &self.task
    }

    pub fn path(&self) -> PathBuf {
        self.domain.tasks().join(&self.task.name)
    }

    pub fn member(&self, name: &str) -> PathBuf {
        self.path().join(name)
    }

    pub fn source(&self, value: &str) -> Result<PathBuf> {
        expand(value, &self.domain.tasks())
    }

    pub fn identity(&self) -> String {
        format!("{}/{}", self.domain.name, self.task.name)
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}
