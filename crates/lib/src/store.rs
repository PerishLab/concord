use crate::model::component;
use crate::path::{at, expand};
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
pub struct TaskRef {
    domain: Domain,
    task: Task,
}

pub(crate) struct Snapshot {
    pub raw: Vec<u8>,
    pub registry: Registry,
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
            if domain.registry_path().is_file() {
                found.push(domain);
            }
        }
        found.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(found)
    }

    pub fn domain(&self, name: &str) -> Result<Domain> {
        let domain = Domain::new(&self.root, name)?;
        if !domain.registry_path().is_file() {
            return Err(Error::new(format!("unknown managed domain {name}")));
        }
        Ok(domain)
    }

    pub fn resolve(&self, identity: &str) -> Result<TaskRef> {
        if let Some((domain, task)) = identity.split_once('/') {
            return self.domain(domain)?.task(task);
        }
        if let Some(domain) = self.containing_domain()?
            && let Ok(task) = domain.task(identity)
        {
            return Ok(task);
        }
        let mut found = Vec::new();
        for domain in self.domains()? {
            if let Ok(task) = domain.task(identity) {
                found.push(task);
            }
        }
        match found.len() {
            0 => Err(Error::new(format!("task not found: {identity}"))),
            1 => Ok(found.remove(0)),
            _ => Err(Error::new(format!(
                "task name is ambiguous; use domain/{identity}"
            ))),
        }
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

    pub(crate) fn task_identity(&self, identity: &str) -> Result<String> {
        if identity.contains('/') {
            return Ok(identity.to_string());
        }
        let domain = self.containing_domain()?.ok_or_else(|| {
            Error::new("task creation outside a managed domain requires domain/task identity")
        })?;
        Ok(format!("{}/{}", domain.name(), identity))
    }

    fn containing_domain(&self) -> Result<Option<Domain>> {
        let current = crate::config::current_dir()?;
        let mut found = self
            .domains()?
            .into_iter()
            .filter(|domain| {
                domain
                    .path()
                    .canonicalize()
                    .is_ok_and(|path| current.starts_with(path))
            })
            .collect::<Vec<_>>();
        found.sort_by_key(|domain| std::cmp::Reverse(domain.path().as_os_str().len()));
        Ok(found.into_iter().next())
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

    pub fn tasks_path(&self) -> PathBuf {
        self.root.join(".tasks")
    }

    pub fn registry_path(&self) -> PathBuf {
        self.tasks_path().join("tasks.toml")
    }

    pub fn task(&self, name: &str) -> Result<TaskRef> {
        component("task name", name)?;
        let snapshot = self.read()?;
        let task = snapshot
            .registry
            .task
            .into_iter()
            .find(|task| task.name == name)
            .ok_or_else(|| Error::new(format!("task not found: {}/{}", self.name, name)))?;
        Ok(TaskRef {
            domain: self.clone(),
            task,
        })
    }

    pub fn registry(&self) -> Result<Registry> {
        Ok(self.read()?.registry)
    }

    pub(crate) fn read(&self) -> Result<Snapshot> {
        let path = self.registry_path();
        let raw = std::fs::read(&path).map_err(|error| {
            Error::new(format!("cannot read registry {}: {error}", path.display()))
        })?;
        let text = std::str::from_utf8(&raw)
            .map_err(|_| Error::new(format!("registry is not utf8: {}", path.display())))?;
        let registry: Registry = toml::from_str(text)?;
        registry.validate()?;
        Ok(Snapshot { raw, registry })
    }

    pub(crate) fn write(&self, before: &[u8], registry: &Registry) -> Result<()> {
        let path = self.registry_path();
        let current = std::fs::read(&path)?;
        if current != before {
            return Err(Error::new(format!(
                "registry changed concurrently: {}",
                path.display()
            )));
        }
        let text = toml::to_string_pretty(registry)?;
        at(&path).file(&text)
    }

    pub(crate) fn bootstrap(&self) -> Result<()> {
        if self.registry_path().exists() {
            return Err(Error::new(format!("domain already managed: {}", self.name)));
        }
        std::fs::create_dir_all(&self.root)?;
        at(&self.tasks_path()).directory()?;
        at(&self.registry_path()).file(&toml::to_string_pretty(&Registry::empty())?)
    }
}

impl TaskRef {
    pub fn domain(&self) -> &Domain {
        &self.domain
    }

    pub fn task(&self) -> &Task {
        &self.task
    }

    pub fn path(&self) -> PathBuf {
        self.domain.tasks_path().join(&self.task.name)
    }

    pub fn member_path(&self, name: &str) -> PathBuf {
        self.path().join(name)
    }

    pub fn source(&self, value: &str) -> Result<PathBuf> {
        expand(value, &self.domain.tasks_path())
    }

    pub fn identity(&self) -> String {
        format!("{}/{}", self.domain.name, self.task.name)
    }

    pub(crate) fn lock(&self) -> Result<Lock> {
        let space = self
            .domain
            .path()
            .parent()
            .ok_or_else(|| Error::new("managed domain has no domain-space parent"))?;
        Space {
            root: space.to_path_buf(),
        }
        .lock()
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}
