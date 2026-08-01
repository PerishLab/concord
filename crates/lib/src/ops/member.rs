use super::{Add, Plan, action};
use crate::git;
use crate::model::component;
use crate::{Domain, Error, Member, Result, Space, TaskRef, claim};
use std::collections::BTreeMap;
use std::path::Path;

impl Space {
    pub fn member_add(&self, request: Add<'_>, apply: bool) -> Result<Plan> {
        component("member name", request.name)?;
        let task = self.resolve(request.task)?;
        write::version(&task)?;
        if request.orphan {
            return Err(Error::new(
                "claimed registry boundary proofs do not support orphan members",
            ));
        }
        task.ensure_exact()?;
        crate::audit::resource::ensure_expansion_headroom(&task.path())?;
        let source = request.source.canonicalize().map_err(|error| {
            Error::new(format!(
                "cannot resolve source {}: {error}",
                request.source.display()
            ))
        })?;
        let write = claim::normalize(request.write)?;
        claim::available(
            self,
            claim::Wanted {
                task: &task.identity(),
                member: request.name,
                source: &source,
                write: &write,
            },
        )?;
        if !git::at(&source).clean()? {
            return Err(Error::new("integration checkout is not clean"));
        }
        let branch = request.branch.unwrap_or(&task.task().name);
        ensure_branch_absent(&source, branch)?;
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
            write::version(&task)?;
            task.ensure_exact()?;
            claim::available(
                self,
                claim::Wanted {
                    task: &task.identity(),
                    member: request.name,
                    source: &source,
                    write: &write,
                },
            )?;
            ensure_branch_absent(&source, branch)?;
            add_member(
                &task,
                request,
                Seat {
                    source: &source,
                    branch,
                    path: &path,
                },
            )?;
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
        ensure_boundary(&task, member, &path)?;
        if !git::at(&path).clean()? {
            return Err(Error::new("member has dirty or untracked files"));
        }
        let source = task.source(&member.source)?;
        if git::at(&path).landing(&source)?.is_none() {
            return Err(Error::new(
                "member HEAD is neither reachable nor tree-equivalent to the integration checkout HEAD",
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
            let member = task
                .task()
                .repo
                .iter()
                .find(|member| member.name == name)
                .ok_or_else(|| Error::new(format!("member not found: {name}")))?;
            ensure_boundary(&task, member, &path)?;
            remove_member(&task, name, &source, &path)?;
        }
        Ok(Plan::new("member.remove-landed", actions, apply))
    }
}

fn ensure_branch_absent(source: &Path, branch: &str) -> Result<()> {
    if git::at(source).exists(branch)? {
        return Err(Error::new(format!(
            "target branch already exists: {branch}"
        )));
    }
    Ok(())
}

struct Seat<'a> {
    source: &'a Path,
    branch: &'a str,
    path: &'a Path,
}

fn add_member(task: &TaskRef, request: Add<'_>, seat: Seat<'_>) -> Result<()> {
    if seat.path.exists() {
        return Err(Error::new(format!(
            "member path already exists: {}",
            seat.path.display()
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
    git::at(seat.source).add(seat.path, seat.branch, request.orphan)?;
    held.repo.push(Member {
        name: request.name.to_string(),
        source: source_text(domain, seat.source),
        branch: (seat.branch != task.task().name).then(|| seat.branch.to_string()),
        write: claim::normalize(request.write)?,
        boundary: None,
        extra: BTreeMap::new(),
    });
    if let Err(error) = domain.write(&snapshot.raw, &snapshot.registry) {
        let _ = git::at(seat.source).remove(seat.path);
        return Err(error);
    }
    Ok(())
}

fn ensure_boundary(task: &TaskRef, member: &Member, path: &Path) -> Result<()> {
    if task.domain().registry()?.version == 1 {
        return Ok(());
    }
    let head = git::at(path).head()?;
    if member
        .boundary
        .as_ref()
        .is_some_and(|proof| crate::boundary::valid(proof, &member.write, &head))
    {
        Ok(())
    } else {
        Err(Error::new(format!(
            "member {} has no valid boundary proof; run concord member boundary {} {}",
            member.name,
            task.identity(),
            member.name
        )))
    }
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
    git::at(source).remove(path)?;
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
mod write;
