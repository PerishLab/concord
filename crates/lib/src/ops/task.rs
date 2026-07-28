use super::{Plan, action};
use crate::git;
use crate::model::component;
use crate::path::private_dir;
use crate::{Domain, Error, Registry, Result, Space, Task, TaskRef};
use std::path::Path;

impl Space {
    pub fn task_start(&self, identity: &str, apply: bool) -> Result<Plan> {
        let identity = self.task_identity(identity)?;
        let identity = identity.as_str();
        let (domain_name, task_name) = qualified(identity)?;
        component("task name", task_name)?;
        let domain = self.domain(domain_name)?;
        let root = domain.tasks_path().join(task_name);
        let actions = vec![
            action("create", &root, "private task root"),
            action(
                "append",
                &domain.registry_path(),
                format!("repo-less task {task_name}"),
            ),
        ];
        if apply {
            let _lock = self.lock()?;
            let mut snapshot = domain.read()?;
            if snapshot
                .registry
                .task
                .iter()
                .any(|task| task.name == task_name)
                || root.exists()
            {
                return Err(Error::new(format!("task already exists: {identity}")));
            }
            private_dir(&root)?;
            snapshot
                .registry
                .task
                .push(Task::new(task_name.to_string()));
            if let Err(error) = domain.write(&snapshot.raw, &snapshot.registry) {
                let _ = std::fs::remove_dir(&root);
                return Err(error);
            }
        }
        Ok(Plan::new("task.start", actions, apply))
    }

    pub fn task_rename(&self, identity: &str, name: &str, apply: bool) -> Result<Plan> {
        component("task name", name)?;
        let task = self.resolve(identity)?;
        task.ensure_exact()?;
        let from = task.path();
        let to = task.domain().tasks_path().join(name);
        let actions = vec![
            action("move", &from, format!("task root -> {}", to.display())),
            action(
                "replace",
                &task.domain().registry_path(),
                format!("task identity {} -> {name}", task.task().name),
            ),
        ];
        if apply {
            let _lock = self.lock()?;
            let task = self.resolve(identity)?;
            task.ensure_exact()?;
            rename(&task, name, &to)?;
        }
        Ok(Plan::new("task.rename", actions, apply))
    }

    pub fn task_rehome(&self, identity: &str, target: &str, apply: bool) -> Result<Plan> {
        let task = self.resolve(identity)?;
        task.ensure_exact()?;
        let domain = self.domain(target)?;
        let from = task.path();
        let to = domain.tasks_path().join(&task.task().name);
        let actions = vec![
            action("move", &from, format!("task root -> {}", to.display())),
            action(
                "transfer",
                &task.domain().registry_path(),
                format!("task entry -> {}", domain.registry_path().display()),
            ),
        ];
        if apply {
            let _lock = self.lock()?;
            let task = self.resolve(identity)?;
            task.ensure_exact()?;
            rehome(&task, &domain, &to)?;
        }
        Ok(Plan::new("task.rehome", actions, apply))
    }

    pub fn task_finish(&self, identity: &str, apply: bool) -> Result<Plan> {
        let task = self.resolve(identity)?;
        task.ensure_exact()?;
        if !task.task().repo.is_empty() {
            return Err(Error::new("task finish requires zero repository members"));
        }
        let mut entries = std::fs::read_dir(task.path())?;
        if entries.next().is_some() {
            return Err(Error::new(
                "task root retains memory or artifacts; remove exact targets first",
            ));
        }
        let actions = vec![
            action("remove", &task.path(), "empty task root"),
            action(
                "remove",
                &task.domain().registry_path(),
                format!("task entry {}", task.task().name),
            ),
        ];
        if apply {
            let _lock = self.lock()?;
            let task = self.resolve(identity)?;
            task.ensure_exact()?;
            finish(&task)?;
        }
        Ok(Plan::new("task.finish", actions, apply))
    }
}

fn rename(task: &TaskRef, name: &str, to: &Path) -> Result<()> {
    if to.exists() {
        return Err(Error::new(format!(
            "target task root already exists: {}",
            to.display()
        )));
    }
    let domain = task.domain();
    let mut snapshot = domain.read()?;
    if snapshot.registry.task.iter().any(|held| held.name == name) {
        return Err(Error::new(format!("target task already exists: {name}")));
    }
    let held = snapshot
        .registry
        .task
        .iter_mut()
        .find(|held| held.name == task.task().name)
        .ok_or_else(|| Error::new("task disappeared during rename"))?;
    let old = held.name.clone();
    held.name = name.to_string();
    for member in &mut held.repo {
        if member.branch.is_none() {
            member.branch = Some(old.clone());
        }
    }
    let repairs = repairs(task, to)?;
    std::fs::rename(task.path(), to)?;
    if let Err(error) = repair_all(&repairs, true) {
        let _ = std::fs::rename(to, task.path());
        let _ = repair_all(&repairs, false);
        return Err(error);
    }
    if let Err(error) = domain.write(&snapshot.raw, &snapshot.registry) {
        let _ = std::fs::rename(to, task.path());
        let _ = repair_all(&repairs, false);
        return Err(error);
    }
    Ok(())
}

fn rehome(task: &TaskRef, target: &Domain, to: &Path) -> Result<()> {
    if task.domain().name() == target.name() {
        return Err(Error::new("task already belongs to target domain"));
    }
    if to.exists() {
        return Err(Error::new(format!(
            "target task root already exists: {}",
            to.display()
        )));
    }
    let source = task.domain();
    let mut before = source.read()?;
    let mut after = target.read()?;
    if after
        .registry
        .task
        .iter()
        .any(|held| held.name == task.task().name)
    {
        return Err(Error::new("target registry already contains task"));
    }
    let index = before
        .registry
        .task
        .iter()
        .position(|held| held.name == task.task().name)
        .ok_or_else(|| Error::new("task disappeared during rehome"))?;
    let mut moved = before.registry.task.remove(index);
    for member in &mut moved.repo {
        member.source = task.source(&member.source)?.display().to_string();
    }
    let repairs = repairs(task, to)?;
    after.registry.task.push(moved);
    std::fs::rename(task.path(), to)?;
    if let Err(error) = repair_all(&repairs, true) {
        let _ = std::fs::rename(to, task.path());
        let _ = repair_all(&repairs, false);
        return Err(error);
    }
    if let Err(error) = target.write(&after.raw, &after.registry) {
        let _ = std::fs::rename(to, task.path());
        let _ = repair_all(&repairs, false);
        return Err(error);
    }
    if let Err(error) = source.write(&before.raw, &before.registry) {
        let now = target.read()?;
        if let Ok(text) = std::str::from_utf8(&after.raw)
            && let Ok(original) = toml::from_str::<Registry>(text)
        {
            let _ = target.write(&now.raw, &original);
        }
        let _ = std::fs::rename(to, task.path());
        let _ = repair_all(&repairs, false);
        return Err(error);
    }
    Ok(())
}

fn finish(task: &TaskRef) -> Result<()> {
    let domain = task.domain();
    let mut snapshot = domain.read()?;
    let index = snapshot
        .registry
        .task
        .iter()
        .position(|held| held.name == task.task().name)
        .ok_or_else(|| Error::new("task disappeared during finish"))?;
    snapshot.registry.task.remove(index);
    std::fs::remove_dir(task.path())?;
    if let Err(error) = domain.write(&snapshot.raw, &snapshot.registry) {
        let _ = private_dir(&task.path());
        return Err(error);
    }
    Ok(())
}

type Repair = (std::path::PathBuf, std::path::PathBuf, std::path::PathBuf);

fn repairs(task: &TaskRef, to: &Path) -> Result<Vec<Repair>> {
    task.task()
        .repo
        .iter()
        .map(|member| {
            Ok((
                task.source(&member.source)?,
                task.member_path(&member.name),
                to.join(&member.name),
            ))
        })
        .collect()
}

fn repair_all(repairs: &[Repair], forward: bool) -> Result<()> {
    for (source, from, to) in repairs {
        git::repair(source, if forward { to } else { from })?;
    }
    Ok(())
}

fn qualified(identity: &str) -> Result<(&str, &str)> {
    identity
        .split_once('/')
        .filter(|(domain, task)| !domain.is_empty() && !task.is_empty() && !task.contains('/'))
        .ok_or_else(|| Error::new("task creation requires domain/task identity"))
}
