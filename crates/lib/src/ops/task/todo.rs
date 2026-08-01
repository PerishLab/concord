use crate::path::at;
use crate::protocol::model::component;
use crate::{Domain, Error, Registry, Result, Space, Task, TaskRef};

use super::super::{Action, Plan, action};

pub(super) struct Todos<'a> {
    registry: &'a Registry,
}

struct Todo<'a> {
    source: &'a TaskRef,
    target: String,
}

impl Space {
    pub fn task_todo_add(&self, identity: &str, value: &str, apply: bool) -> Result<Plan> {
        let source = self.resolve(identity)?;
        source.ensure_exact()?;
        let todo = Todo::new(&source, value)?;
        let registry = source.domain().registry()?;
        let todos = Todos::writable(&registry)?;
        let exists = todos.exists(&todo.target);
        if exists {
            source.domain().task(&todo.target)?.ensure_exact()?;
        } else if source.domain().tasks_path().join(&todo.target).exists() {
            return Err(Error::new(format!(
                "todo target root has no registry entry: {}",
                todo.target
            )));
        }
        let actions = todo.actions(exists);
        if apply {
            let _lock = self.lock()?;
            let source = self.resolve(identity)?;
            source.ensure_exact()?;
            Todo::new(&source, value)?.add()?;
        }
        Ok(Plan::new("task.todo.add", actions, apply))
    }

    pub fn task_todo_remove(&self, identity: &str, value: &str, apply: bool) -> Result<Plan> {
        let source = self.resolve(identity)?;
        source.ensure_exact()?;
        let todo = Todo::new(&source, value)?;
        let registry = source.domain().registry()?;
        Todos::writable(&registry)?.link(&source.task().name, &todo.target)?;
        source.domain().task(&todo.target)?.ensure_exact()?;
        let actions = vec![action(
            "unlink",
            &source.domain().registry_path(),
            format!("task {} todo {}", source.task().name, todo.target),
        )];
        if apply {
            let _lock = self.lock()?;
            let source = self.resolve(identity)?;
            source.ensure_exact()?;
            Todo::new(&source, value)?.remove()?;
        }
        Ok(Plan::new("task.todo.remove", actions, apply))
    }
}

impl<'a> Todos<'a> {
    pub(super) fn new(registry: &'a Registry) -> Self {
        Self { registry }
    }

    fn writable(registry: &'a Registry) -> Result<Self> {
        if registry.version != 3 {
            return Err(Error::new(format!(
                "registry version {} requires explicit migration before task todo mutation",
                registry.version
            )));
        }
        Ok(Self::new(registry))
    }

    pub(super) fn incoming(&self, name: &str) -> Vec<String> {
        self.registry
            .task
            .iter()
            .filter(|task| task.todo.iter().any(|target| target == name))
            .map(|task| task.name.clone())
            .collect()
    }

    pub(super) fn unlinked(&self, name: &str) -> Result<()> {
        let incoming = self.incoming(name);
        let outgoing = self
            .registry
            .task
            .iter()
            .find(|task| task.name == name)
            .map(|task| task.todo.as_slice())
            .unwrap_or_default();
        if incoming.is_empty() && outgoing.is_empty() {
            return Ok(());
        }
        Err(Error::new(format!(
            "task {name} has todo links; remove or hand them off before rehoming"
        )))
    }

    fn exists(&self, name: &str) -> bool {
        self.registry.task.iter().any(|task| task.name == name)
    }

    fn link(&self, source: &str, target: &str) -> Result<()> {
        let linked = self
            .registry
            .task
            .iter()
            .find(|task| task.name == source)
            .is_some_and(|task| task.todo.iter().any(|held| held == target));
        if linked {
            Ok(())
        } else {
            Err(Error::new(format!("task {source} has no todo {target}")))
        }
    }
}

impl<'a> Todo<'a> {
    fn new(source: &'a TaskRef, value: &str) -> Result<Self> {
        let name = match value.split_once('/') {
            Some((domain, name)) => {
                if domain != source.domain().name() || name.contains('/') {
                    return Err(Error::new(
                        "task todos are same-domain in registry version 3",
                    ));
                }
                name
            }
            None => value,
        };
        component("todo task name", name)?;
        if name == source.task().name {
            return Err(Error::new("a task cannot reference itself as a todo"));
        }
        Ok(Self {
            source,
            target: name.to_string(),
        })
    }

    fn actions(&self, exists: bool) -> Vec<Action> {
        if self
            .source
            .task()
            .todo
            .iter()
            .any(|held| held == &self.target)
        {
            return vec![action(
                "keep",
                &self.source.domain().registry_path(),
                format!(
                    "task {} already has todo {}",
                    self.source.task().name,
                    self.target
                ),
            )];
        }
        let mut actions = Vec::new();
        if !exists {
            actions.push(action(
                "create",
                &self.source.domain().tasks_path().join(&self.target),
                "private repo-less todo task root",
            ));
        }
        actions.push(action(
            "link",
            &self.source.domain().registry_path(),
            format!("task {} todo {}", self.source.task().name, self.target),
        ));
        actions
    }

    fn add(&self) -> Result<()> {
        let domain = self.source.domain();
        let mut snapshot = domain.read()?;
        let todos = Todos::writable(&snapshot.registry)?;
        let index = snapshot
            .registry
            .task
            .iter()
            .position(|task| task.name == self.source.task().name)
            .ok_or_else(|| Error::new("todo source disappeared during add"))?;
        let exists = todos.exists(&self.target);
        if exists {
            domain.task(&self.target)?.ensure_exact()?;
        }
        if snapshot.registry.task[index]
            .todo
            .iter()
            .any(|held| held == &self.target)
        {
            return Ok(());
        }
        let root = domain.tasks_path().join(&self.target);
        if !exists {
            if root.exists() {
                return Err(Error::new(format!(
                    "todo target root has no registry entry: {}",
                    self.target
                )));
            }
            at(&root).directory()?;
            snapshot.registry.task.push(Task::new(self.target.clone()));
        }
        snapshot.registry.task[index].todo.push(self.target.clone());
        snapshot.registry.task[index].todo.sort();
        if let Err(error) = snapshot
            .registry
            .validate()
            .and_then(|()| domain.write(&snapshot.raw, &snapshot.registry))
        {
            if !exists {
                let _ = std::fs::remove_dir(&root);
            }
            return Err(error);
        }
        Ok(())
    }

    fn remove(&self) -> Result<()> {
        let domain = self.source.domain();
        let mut snapshot = domain.read()?;
        Todos::writable(&snapshot.registry)?.link(&self.source.task().name, &self.target)?;
        domain.task(&self.target)?.ensure_exact()?;
        let task = snapshot
            .registry
            .task
            .iter_mut()
            .find(|task| task.name == self.source.task().name)
            .ok_or_else(|| Error::new("todo source disappeared during remove"))?;
        task.todo.retain(|held| held != &self.target);
        domain.write(&snapshot.raw, &snapshot.registry)
    }
}

pub(super) fn rewrite(registry: &mut Registry, from: &str, to: &str) {
    for task in &mut registry.task {
        for target in &mut task.todo {
            if target == from {
                *target = to.to_string();
            }
        }
        task.todo.sort();
    }
}

pub(super) fn handoffs(domain: &Domain, task: &Task) -> Vec<Action> {
    task.todo
        .iter()
        .map(|target| {
            action(
                "handoff",
                &domain.tasks_path().join(target),
                format!("todo from {} remains an ordinary task", task.name),
            )
        })
        .collect()
}
