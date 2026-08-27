use super::{Agreement, Estate, Life, Node};
use crate::git;
use crate::path::at;
use crate::{PLUMB, Result};
use std::collections::BTreeSet;
use std::path::Path;

pub(super) async fn inspect(plane: &Estate, tasks: &[Node], report: &mut Agreement) -> Result<()> {
    custody(plane, report)?;
    let members = plane.worktrees().await?;
    for task in tasks {
        let identity = task.identity();
        let held = members
            .iter()
            .filter(|member| member.task == identity)
            .collect::<Vec<_>>();
        seat(plane, task, &held, report)?;
        for worktree in held {
            member(plane, task, worktree, report);
        }
    }
    overlaps(plane, &members, report)?;
    Ok(())
}

fn custody(state: &Estate, report: &mut Agreement) -> Result<()> {
    for (path, mode) in [
        (state.space.join(".concord"), 0o700),
        (state.space.join(".concord/estate.sqlite3"), 0o600),
        (state.space.join(".concord/sudo"), 0o600),
    ] {
        if let Some(found) = at(&path).held()?
            && found != mode
        {
            report.fault(
                "permission",
                path.display().to_string(),
                format!("mode {found:04o} must be {mode:04o}"),
            );
        }
    }
    Ok(())
}

fn seat(
    plane: &Estate,
    task: &Node,
    members: &[&super::Worktree],
    report: &mut Agreement,
) -> Result<()> {
    let root = plane
        .space
        .join(&task.domain)
        .join(".tasks")
        .join(&task.name);
    if !root.exists() {
        if !members.is_empty() {
            report.fault(
                "member.missing",
                task.identity(),
                "Task Member seat is absent",
            );
        }
        return Ok(());
    }
    let metadata = std::fs::symlink_metadata(&root)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        report.fault(
            "territory.task",
            root.display().to_string(),
            "Task seat is not a direct directory",
        );
        return Ok(());
    }
    let names = members
        .iter()
        .map(|member| member.name.as_str())
        .collect::<BTreeSet<_>>();
    for entry in std::fs::read_dir(&root)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name != ".task" && !names.contains(name.as_ref()) {
            report.fault(
                "territory.unknown",
                entry.path().display().to_string(),
                "undeclared entry occupies the derived Task seat",
            );
        }
    }
    artifacts(&root, report)
}

fn artifacts(task: &Path, report: &mut Agreement) -> Result<()> {
    let memory = task.join(".task");
    if !memory.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(&memory)? {
        let entry = entry?;
        if entry.file_name() != "artifacts" {
            report.fault(
                "territory.memory",
                entry.path().display().to_string(),
                "MAIN/PHASE or unknown memory territory remains active",
            );
        }
    }
    let root = memory.join("artifacts");
    if !root.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        if entry.file_type()?.is_symlink() || !entry.file_type()?.is_dir() {
            report.fault(
                "artifact.territory",
                entry.path().display().to_string(),
                "Artifact seat must be a direct directory",
            );
        }
    }
    Ok(())
}

fn member(state: &Estate, task: &Node, member: &super::Worktree, report: &mut Agreement) {
    let subject = format!("{}/{}", member.task, member.name);
    if task.life == Life::Retired {
        report.fault(
            "member.retired",
            &subject,
            "retired Task retains a live Member",
        );
    }
    let path = state.path(&task.domain, &task.name, &member.name);
    if !valid(state, member, &path) {
        report.fault(
            "member.agreement",
            &subject,
            "Member source, worktree identity, registration, or branch disagrees",
        );
    }
    if crate::claim::normalize(&member.claims).ok().as_ref() != Some(&member.claims) {
        report.fault("claim.shape", &subject, "Member Claims are not normalized");
    }
    if let Some(proof) = &member.proof
        && !current(proof, member, &path)
    {
        report.observe("boundary.stale", subject, "Member Boundary proof is stale");
    }
}

fn overlaps(plane: &Estate, members: &[super::Worktree], report: &mut Agreement) -> Result<()> {
    for (index, left) in members.iter().enumerate() {
        for right in members.iter().skip(index + 1) {
            let same = git::at(&plane.source(left)?).identity()?
                == git::at(&plane.source(right)?).identity()?;
            let paths = crate::claim::intersections(&left.claims, &right.claims);
            if same && !paths.is_empty() {
                report.observe(
                    "claim.overlap",
                    format!("{}/{}", left.task, left.name),
                    format!(
                        "write Claim overlaps {}/{} at {}",
                        right.task,
                        right.name,
                        paths.join(", ")
                    ),
                );
            }
        }
    }
    Ok(())
}

fn valid(state: &Estate, member: &super::Worktree, path: &Path) -> bool {
    let Ok(source) = state.source(member) else {
        return false;
    };
    if !path.is_dir() || git::at(&source).identity().ok() != git::at(path).identity().ok() {
        return false;
    }
    if !git::at(&source).registered(path).unwrap_or(false) {
        return false;
    }
    git::at(path).branch().ok().as_deref() == Some(member.branch.as_str())
}

fn current(proof: &super::Proof, member: &super::Worktree, path: &Path) -> bool {
    if proof.schema != plumb::boundary::SCHEMA || proof.plumb != PLUMB {
        return false;
    }
    if git::at(path).head().ok().as_deref() != Some(proof.head.as_str()) {
        return false;
    }
    proof.claim == crate::claim::digest(&member.claims)
}
