use super::{Plan, action};
use crate::model::component;
use crate::path::mode;
use crate::{Domain, Error, Repo, Result, Space};
use std::collections::BTreeMap;

impl Space {
    pub fn domain_init(&self, name: &str, apply: bool) -> Result<Plan> {
        let domain = Domain::new(self.path(), name)?;
        let actions = vec![
            action("create", &domain.tasks_path(), "private task registry seat"),
            action("write", &domain.registry_path(), "version 1 empty registry"),
        ];
        if apply {
            let _lock = self.lock()?;
            domain.bootstrap()?;
        }
        Ok(Plan::new("domain.init", actions, apply))
    }

    pub fn repo_annotate(
        &self,
        domain: &str,
        name: &str,
        note: Option<&str>,
        apply: bool,
    ) -> Result<Plan> {
        component("repository name", name)?;
        let domain = self.domain(domain)?;
        let actions = vec![action(
            "append",
            &domain.registry_path(),
            format!("repository annotation {name}"),
        )];
        if apply {
            let _lock = self.lock()?;
            let mut snapshot = domain.read()?;
            if snapshot.registry.repo.iter().any(|repo| repo.name == name) {
                return Err(Error::new(format!(
                    "repository annotation already exists: {name}"
                )));
            }
            let mut extra = BTreeMap::new();
            if let Some(note) = note {
                extra.insert("note".to_string(), toml::Value::String(note.to_string()));
            }
            snapshot.registry.repo.push(Repo {
                name: name.to_string(),
                extra,
            });
            domain.write(&snapshot.raw, &snapshot.registry)?;
        }
        Ok(Plan::new("repo.annotate", actions, apply))
    }

    pub fn normalize(&self, identity: &str, apply: bool) -> Result<Plan> {
        let task = self.resolve(identity)?;
        let mut actions = vec![action(
            "chmod",
            &task.path(),
            "task structure 0700 and managed memory private",
        )];
        for link in crate::Memory::new(&task).links()? {
            actions.push(action("skip", &link, "symbolic link is not a mode target"));
        }
        if apply {
            let _lock = self.lock()?;
            let task = self.resolve(identity)?;
            mode(&task.domain().tasks_path(), 0o700)?;
            mode(&task.domain().registry_path(), 0o600)?;
            mode(&task.path(), 0o700)?;
            crate::Memory::new(&task).normalize()?;
        }
        Ok(Plan::new("permissions.normalize", actions, apply))
    }
}
