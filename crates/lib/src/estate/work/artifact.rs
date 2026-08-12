mod preflight;

use super::super::World;
use super::{Estate, active};
use crate::component;
use crate::path::at;
use crate::{Error, Result};
pub use preflight::Survey;
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Artifact {
    pub task: String,
    pub name: String,
    pub path: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Import {
    pub task: String,
    pub name: String,
    pub source: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Removal {
    pub task: String,
    pub name: String,
}

impl Estate {
    pub async fn artifacts(&self, task: &str) -> Result<Vec<Artifact>> {
        let world = World::load(self).await?;
        let task = world.node(task)?;
        let root = self.seat(&task.domain, &task.name);
        if !root.exists() {
            return Ok(Vec::new());
        }
        if !root.is_dir() {
            return Err(foreign(&root));
        }
        let mut found = Vec::new();
        for entry in std::fs::read_dir(root)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                return Err(foreign(&entry.path()));
            }
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| Error::typed("concord.artifact.name", "Artifact name is not UTF-8"))?;
            component("artifact name", &name)?;
            found.push(Artifact {
                task: task.identity(),
                name,
                path: entry.path(),
            });
        }
        found.sort_by_key(|artifact| artifact.name.clone());
        Ok(found)
    }

    pub async fn artifact(&self, task: &str, name: &str) -> Result<Artifact> {
        component("artifact name", name)?;
        self.artifacts(task)
            .await?
            .into_iter()
            .find(|artifact| artifact.name == name)
            .ok_or_else(|| {
                Error::typed(
                    "concord.artifact.absent",
                    format!("Artifact does not exist: {task}/{name}"),
                )
            })
    }

    pub async fn preflight(&self, import: &Import) -> Result<Survey> {
        component("artifact name", &import.name)?;
        let _guard = self.guard()?;
        self.ensure().await?;
        let world = World::load(self).await?;
        let task = world.node(&import.task)?;
        active(task.life, &import.task)?;
        let target = self.seat(&task.domain, &task.name).join(&import.name);
        vacant(&target)?;
        preflight::inspect(&import.source, &target, &self.space)
    }

    pub async fn import(&self, import: &Import) -> Result<Artifact> {
        component("artifact name", &import.name)?;
        let _guard = self.guard()?;
        self.ensure().await?;
        let world = World::load(self).await?;
        let task = world.node(&import.task)?;
        active(task.life, &import.task)?;
        let root = self.seat(&task.domain, &task.name);
        let target = root.join(&import.name);
        vacant(&target)?;
        let checked = preflight::inspect(&import.source, &target, &self.space)?;
        let source = PathBuf::from(checked.source);
        at(&root).directory()?;
        at(&target).directory()?;
        let copied = if source.is_dir() {
            super::copy::tree(&source, &target)
        } else {
            let name = source
                .file_name()
                .ok_or_else(|| Error::new("Artifact source has no filename"))?;
            super::copy::file(&source, &target.join(name))
        };
        if let Err(error) = copied {
            let _ = std::fs::remove_dir_all(&target);
            prune(&root);
            return Err(error);
        }
        Ok(Artifact {
            task: task.identity(),
            name: import.name.clone(),
            path: target,
        })
    }

    pub async fn remove(&self, removal: &Removal) -> Result<()> {
        component("artifact name", &removal.name)?;
        let _guard = self.guard()?;
        self.ensure().await?;
        let world = World::load(self).await?;
        let task = world.node(&removal.task)?;
        active(task.life, &removal.task)?;
        let root = self.seat(&task.domain, &task.name);
        let target = root.join(&removal.name);
        let metadata = std::fs::symlink_metadata(&target).map_err(|error| {
            Error::typed(
                "concord.artifact.absent",
                format!("Artifact does not exist at {}: {error}", target.display()),
            )
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(foreign(&target));
        }
        std::fs::remove_dir_all(&target)?;
        prune(&root);
        Ok(())
    }

    fn seat(&self, domain: &str, task: &str) -> PathBuf {
        self.space
            .join(domain)
            .join(".tasks")
            .join(task)
            .join(".task/artifacts")
    }
}

fn vacant(target: &Path) -> Result<()> {
    if target.exists() || std::fs::symlink_metadata(target).is_ok() {
        return Err(Error::typed(
            "concord.artifact.reserved",
            format!("Artifact seat already exists: {}", target.display()),
        ));
    }
    Ok(())
}

fn foreign(path: &Path) -> Error {
    Error::typed(
        "concord.artifact.territory",
        format!(
            "Artifact seat contains foreign territory: {}",
            path.display()
        ),
    )
}

fn prune(root: &Path) {
    let Some(memory) = root.parent() else {
        return;
    };
    let Some(task) = memory.parent() else {
        return;
    };
    for path in [root, memory, task] {
        let empty = path
            .read_dir()
            .ok()
            .and_then(|mut entries| entries.next())
            .is_none();
        if path.is_dir() && empty {
            let _ = std::fs::remove_dir(path);
        }
    }
}
