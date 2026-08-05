mod brief;
mod seat;
mod todo;

pub use brief::{
    TASK_BRIEF_LIMIT, TASK_BRIEF_SECTION_BYTES, TaskBriefEntry, TaskBriefLimits, TaskBriefPage,
};

use super::{Plan, action};
use crate::model::component;
use crate::path::at;
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
            at(&root).directory()?;
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
        let registry = task.domain().registry()?;
        let mut actions = vec![
            action("move", &from, format!("task root -> {}", to.display())),
            action(
                "replace",
                &task.domain().registry_path(),
                format!("task identity {} -> {name}", task.task().name),
            ),
        ];
        for source in todo::Todos::new(&registry).incoming(&task.task().name) {
            actions.push(action(
                "rewrite",
                &task.domain().registry_path(),
                format!("task {source} todo {} -> {name}", task.task().name),
            ));
        }
        if apply {
            let _lock = self.lock()?;
            let task = self.resolve(identity)?;
            task.ensure_exact()?;
            task.rename(name, &to)?;
        }
        Ok(Plan::new("task.rename", actions, apply))
    }

    pub fn task_rehome(&self, identity: &str, target: &str, apply: bool) -> Result<Plan> {
        let task = self.resolve(identity)?;
        task.ensure_exact()?;
        todo::Todos::new(&task.domain().registry()?).unlinked(&task.task().name)?;
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
            task.rehome(&domain, &to)?;
        }
        Ok(Plan::new("task.rehome", actions, apply))
    }

    pub fn task_finish(&self, identity: &str, apply: bool) -> Result<Plan> {
        let task = self.resolve(identity)?;
        task.ensure_exact()?;
        if !task.task().repo.is_empty() {
            return Err(Error::new("task finish requires zero repository members"));
        }
        let registry = task.domain().registry()?;
        let incoming = todo::Todos::new(&registry).incoming(&task.task().name);
        if !incoming.is_empty() {
            return Err(Error::new(format!(
                "task is still referenced as a todo by {}",
                incoming.join(", ")
            )));
        }
        let mut entries = std::fs::read_dir(task.path())?;
        if entries.next().is_some() {
            return Err(Error::new(
                "task root retains memory or artifacts; remove exact targets first",
            ));
        }
        let mut actions = todo::handoffs(task.domain(), task.task());
        actions.extend([
            action("remove", &task.path(), "empty task root"),
            action(
                "remove",
                &task.domain().registry_path(),
                format!("task entry {}", task.task().name),
            ),
        ]);
        if apply {
            let _lock = self.lock()?;
            let task = self.resolve(identity)?;
            task.ensure_exact()?;
            task.finish()?;
        }
        Ok(Plan::new("task.finish", actions, apply))
    }
}

impl TaskRef {
    fn rename(&self, name: &str, to: &Path) -> Result<()> {
        if to.exists() {
            return Err(Error::new(format!(
                "target task root already exists: {}",
                to.display()
            )));
        }
        let domain = self.domain();
        let mut snapshot = domain.read()?;
        if snapshot.registry.task.iter().any(|held| held.name == name) {
            return Err(Error::new(format!("target task already exists: {name}")));
        }
        let old = {
            let held = snapshot
                .registry
                .task
                .iter_mut()
                .find(|held| held.name == self.task().name)
                .ok_or_else(|| Error::new("task disappeared during rename"))?;
            let old = held.name.clone();
            held.name = name.to_string();
            for member in &mut held.repo {
                if member.branch.is_none() {
                    member.branch = Some(old.clone());
                }
            }
            old
        };
        todo::rewrite(&mut snapshot.registry, &old, name);
        let repairs = seat::repairs(self, to)?;
        std::fs::rename(self.path(), to)?;
        if let Err(error) = seat::apply(&repairs, true) {
            let _ = std::fs::rename(to, self.path());
            let _ = seat::apply(&repairs, false);
            return Err(error);
        }
        if let Err(error) = domain.write(&snapshot.raw, &snapshot.registry) {
            let _ = std::fs::rename(to, self.path());
            let _ = seat::apply(&repairs, false);
            return Err(error);
        }
        Ok(())
    }

    fn rehome(&self, target: &Domain, to: &Path) -> Result<()> {
        if self.domain().name() == target.name() {
            return Err(Error::new("task already belongs to target domain"));
        }
        if to.exists() {
            return Err(Error::new(format!(
                "target task root already exists: {}",
                to.display()
            )));
        }
        let source = self.domain();
        let mut before = source.read()?;
        todo::Todos::new(&before.registry).unlinked(&self.task().name)?;
        let mut after = target.read()?;
        if after
            .registry
            .task
            .iter()
            .any(|held| held.name == self.task().name)
        {
            return Err(Error::new("target registry already contains task"));
        }
        let index = before
            .registry
            .task
            .iter()
            .position(|held| held.name == self.task().name)
            .ok_or_else(|| Error::new("task disappeared during rehome"))?;
        let mut moved = before.registry.task.remove(index);
        for member in &mut moved.repo {
            member.source = self.source(&member.source)?.display().to_string();
        }
        let repairs = seat::repairs(self, to)?;
        after.registry.task.push(moved);
        std::fs::rename(self.path(), to)?;
        if let Err(error) = seat::apply(&repairs, true) {
            let _ = std::fs::rename(to, self.path());
            let _ = seat::apply(&repairs, false);
            return Err(error);
        }
        if let Err(error) = target.write(&after.raw, &after.registry) {
            let _ = std::fs::rename(to, self.path());
            let _ = seat::apply(&repairs, false);
            return Err(error);
        }
        if let Err(error) = source.write(&before.raw, &before.registry) {
            let now = target.read()?;
            if let Ok(text) = std::str::from_utf8(&after.raw)
                && let Ok(original) = toml::from_str::<Registry>(text)
            {
                let _ = target.write(&now.raw, &original);
            }
            let _ = std::fs::rename(to, self.path());
            let _ = seat::apply(&repairs, false);
            return Err(error);
        }
        Ok(())
    }

    fn finish(&self) -> Result<()> {
        let domain = self.domain();
        let mut snapshot = domain.read()?;
        let incoming = todo::Todos::new(&snapshot.registry).incoming(&self.task().name);
        if !incoming.is_empty() {
            return Err(Error::new(format!(
                "task is still referenced as a todo by {}",
                incoming.join(", ")
            )));
        }
        let index = snapshot
            .registry
            .task
            .iter()
            .position(|held| held.name == self.task().name)
            .ok_or_else(|| Error::new("task disappeared during finish"))?;
        snapshot.registry.task.remove(index);
        std::fs::remove_dir(self.path())?;
        if let Err(error) = domain.write(&snapshot.raw, &snapshot.registry) {
            let _ = at(&self.path()).directory();
            return Err(error);
        }
        Ok(())
    }
}

fn qualified(identity: &str) -> Result<(&str, &str)> {
    identity
        .split_once('/')
        .filter(|(domain, task)| !domain.is_empty() && !task.is_empty() && !task.contains('/'))
        .ok_or_else(|| Error::new("task creation requires domain/task identity"))
}
