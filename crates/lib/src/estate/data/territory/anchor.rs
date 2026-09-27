use super::super::{Agreement, Anchor, Estate, IssueWorktree, Proof, fault};
use crate::{PLUMB, Result, git};
use std::collections::BTreeSet;
use std::path::Path;

pub(super) async fn inspect(plane: &Estate, report: &mut Agreement) -> Result<()> {
    if !plane.execution {
        return Ok(());
    }
    let anchors = plane.core.live("Anchor").await.map_err(fault)?;
    let members = plane.issue_worktrees().await?;
    for row in &anchors {
        let anchor = match super::super::forge::decode_anchor(row) {
            Ok(anchor) => anchor,
            Err(_) => continue,
        };
        let held = members
            .iter()
            .filter(|member| member.node == anchor.node)
            .collect::<Vec<_>>();
        seat(plane, &anchor, &held, report)?;
        for member in held {
            agreement(plane, &anchor, member, report);
        }
    }
    relations(plane, report).await?;
    overlaps(plane, &members, report)?;
    if let Err(error) = crate::occupancy::readable(&plane.space) {
        report.observe(
            "occupancy.read",
            plane.space.display().to_string(),
            error.to_string(),
        );
    }
    Ok(())
}

fn seat(
    state: &Estate,
    anchor: &Anchor,
    held: &[&IssueWorktree],
    report: &mut Agreement,
) -> Result<()> {
    let root = state.space.join(".issues").join(&anchor.node);
    if !root.exists() {
        if !held.is_empty() {
            report.fault(
                "member.missing",
                anchor.coordinate.identity(),
                "Issue Member seat is absent",
            );
        }
        return Ok(());
    }
    let metadata = std::fs::symlink_metadata(&root)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        report.fault(
            "territory.issue",
            root.display().to_string(),
            "Issue seat is not a direct directory",
        );
        return Ok(());
    }
    for entry in std::fs::read_dir(&root)? {
        let entry = entry?;
        let name = entry.file_name();
        if name != "members" && name != "artifacts" {
            report.fault(
                "territory.unknown",
                entry.path().display().to_string(),
                "undeclared entry occupies the derived Issue seat",
            );
        }
    }
    members(&root.join("members"), held, report)?;
    artifacts(&root.join("artifacts"), report)
}

fn members(root: &Path, held: &[&IssueWorktree], report: &mut Agreement) -> Result<()> {
    if !root.exists() {
        return Ok(());
    }
    let names = held
        .iter()
        .map(|member| member.name.as_str())
        .collect::<BTreeSet<_>>();
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        let name = entry.file_name();
        if entry.file_type()?.is_symlink()
            || !entry.file_type()?.is_dir()
            || !names.contains(name.to_string_lossy().as_ref())
        {
            report.fault(
                "territory.unknown",
                entry.path().display().to_string(),
                "undeclared entry occupies the Issue Member seat",
            );
        }
    }
    Ok(())
}

fn artifacts(root: &Path, report: &mut Agreement) -> Result<()> {
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

fn agreement(plane: &Estate, anchor: &Anchor, member: &IssueWorktree, report: &mut Agreement) {
    let subject = format!("{}/{}", anchor.coordinate.identity(), member.name);
    let path = plane
        .space
        .join(".issues")
        .join(&anchor.node)
        .join("members")
        .join(&member.name);
    if !valid(plane, member, &path) {
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

async fn relations(state: &Estate, report: &mut Agreement) -> Result<()> {
    let members = state.core.live("IssueMember").await.map_err(fault)?;
    for (unit, relation) in [
        ("IssueClaim", "member"),
        ("IssueBoundary", "member"),
        ("IssueChange", "member"),
    ] {
        for row in state.core.live(unit).await.map_err(fault)? {
            let expected = row.int(relation).and_then(|key| {
                members
                    .iter()
                    .find(|member| member.key() == key)
                    .and_then(|member| member.int("anchor"))
            });
            if expected.is_none() || row.int("anchor") != expected {
                report.fault(
                    "issue.relation",
                    format!("{unit}/{}", row.key()),
                    "execution resource does not directly agree with its Issue anchor",
                );
            }
        }
    }
    Ok(())
}

fn overlaps(plane: &Estate, members: &[IssueWorktree], report: &mut Agreement) -> Result<()> {
    for (index, left) in members.iter().enumerate() {
        for right in members.iter().skip(index + 1) {
            let same = git::at(&plane.issue_source(left)?).identity()?
                == git::at(&plane.issue_source(right)?).identity()?;
            let paths = crate::claim::intersections(&left.claims, &right.claims);
            if same && !paths.is_empty() {
                report.observe(
                    "claim.overlap",
                    format!("{}/{}", left.issue.identity(), left.name),
                    format!(
                        "write Claim overlaps {}/{} at {}",
                        right.issue.identity(),
                        right.name,
                        paths.join(", ")
                    ),
                );
            }
        }
    }
    Ok(())
}

fn valid(state: &Estate, member: &IssueWorktree, path: &Path) -> bool {
    let Ok(source) = state.issue_source(member) else {
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

fn current(proof: &Proof, member: &IssueWorktree, path: &Path) -> bool {
    if proof.schema != plumb::boundary::SCHEMA || proof.plumb != PLUMB {
        return false;
    }
    if git::at(path).head().ok().as_deref() != Some(proof.head.as_str()) {
        return false;
    }
    proof.claim == crate::claim::digest(&member.claims)
}
