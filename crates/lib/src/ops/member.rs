use super::{Add, Plan, action};
use crate::git;
use crate::model::component;
use crate::{Domain, Error, Member, Result, Space, TaskRef};
use std::collections::BTreeMap;
use std::path::Path;

impl Space {
    pub fn member_add(&self, request: Add<'_>, apply: bool) -> Result<Plan> {
        component("member name", request.name)?;
        let task = self.resolve(request.task)?;
        task.ensure_exact()?;
        let source = request.source.canonicalize().map_err(|error| {
            Error::new(format!(
                "cannot resolve source {}: {error}",
                request.source.display()
            ))
        })?;
        if !git::clean(&source)? {
            return Err(Error::new("integration checkout is not clean"));
        }
        let branch = request.branch.unwrap_or(&task.task().name);
        let path = task.member_path(request.name);
        let actions = vec![
            action(
                "worktree",
                &path,
                format!("source {} branch {branch}", source.display()),
            ),
            action(
                "append",
                &task.domain().registry_path(),
                format!("member {}/{}", task.task().name, request.name),
            ),
        ];
        if apply {
            let _lock = self.lock()?;
            let task = self.resolve(request.task)?;
            task.ensure_exact()?;
            add_member(&task, request, &source, branch, &path)?;
        }
        Ok(Plan::new("member.add", actions, apply))
    }

    pub fn member_remove(&self, identity: &str, name: &str, apply: bool) -> Result<Plan> {
        let task = self.resolve(identity)?;
        task.ensure_exact()?;
        let member = task
            .task()
            .repo
            .iter()
            .find(|member| member.name == name)
            .ok_or_else(|| Error::new(format!("member not found: {name}")))?;
        let path = task.member_path(name);
        if !git::clean(&path)? {
            return Err(Error::new("member has dirty or untracked files"));
        }
        let source = task.source(&member.source)?;
        if !git::reachable_from_head(&path, &source)? {
            return Err(Error::new(
                "member HEAD is not reachable from the integration checkout HEAD",
            ));
        }
        let actions = vec![
            action("worktree-remove", &path, "clean landed member seat"),
            action(
                "remove",
                &task.domain().registry_path(),
                format!("member {name}"),
            ),
        ];
        if apply {
            let _lock = self.lock()?;
            let task = self.resolve(identity)?;
            task.ensure_exact()?;
            remove_member(&task, name, &source, &path)?;
        }
        Ok(Plan::new("member.remove-landed", actions, apply))
    }
}

fn add_member(
    task: &TaskRef,
    request: Add<'_>,
    source: &Path,
    branch: &str,
    path: &Path,
) -> Result<()> {
    if path.exists() {
        return Err(Error::new(format!(
            "member path already exists: {}",
            path.display()
        )));
    }
    let domain = task.domain();
    let mut snapshot = domain.read()?;
    let held = snapshot
        .registry
        .task
        .iter_mut()
        .find(|held| held.name == task.task().name)
        .ok_or_else(|| Error::new("task disappeared during member add"))?;
    if held.repo.iter().any(|member| member.name == request.name) {
        return Err(Error::new(format!(
            "member already exists: {}",
            request.name
        )));
    }
    git::add(source, path, branch, request.orphan)?;
    held.repo.push(Member {
        name: request.name.to_string(),
        source: source_text(domain, source),
        branch: (branch != task.task().name).then(|| branch.to_string()),
        extra: BTreeMap::new(),
    });
    if let Err(error) = domain.write(&snapshot.raw, &snapshot.registry) {
        let _ = git::remove(source, path);
        return Err(error);
    }
    Ok(())
}

fn remove_member(task: &TaskRef, name: &str, source: &Path, path: &Path) -> Result<()> {
    let domain = task.domain();
    let mut snapshot = domain.read()?;
    let held = snapshot
        .registry
        .task
        .iter_mut()
        .find(|held| held.name == task.task().name)
        .ok_or_else(|| Error::new("task disappeared during member removal"))?;
    let index = held
        .repo
        .iter()
        .position(|member| member.name == name)
        .ok_or_else(|| Error::new("member disappeared during removal"))?;
    held.repo.remove(index);
    git::remove(source, path)?;
    if let Err(error) = domain.write(&snapshot.raw, &snapshot.registry) {
        return Err(Error::new(format!(
            "{error}; worktree is removed but registry still declares it"
        )));
    }
    Ok(())
}

fn source_text(domain: &Domain, source: &Path) -> String {
    if source.parent() == Some(domain.path())
        && let Some(name) = source.file_name().and_then(|value| value.to_str())
    {
        return format!("../{name}");
    }
    source.display().to_string()
}
