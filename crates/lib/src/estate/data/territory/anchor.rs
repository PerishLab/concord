use super::super::{Agreement, Anchor, Estate, IssueWorktree, Proof, fault};
use crate::{Result, git};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub(super) async fn inspect(plane: &Estate, report: &mut Agreement) -> Result<()> {
    let survey = Survey { estate: plane };
    let anchors = plane.core.live("Anchor").await.map_err(fault)?;
    let members = plane.issue_worktrees().await?;
    survey.integrations(&members, report).await?;
    for row in &anchors {
        let anchor = match super::super::forge::decode_anchor(row) {
            Ok(anchor) => anchor,
            Err(_) => continue,
        };
        let held = members
            .iter()
            .filter(|member| member.node == anchor.node)
            .collect::<Vec<_>>();
        survey.seat(&anchor, &held, report)?;
        for member in held {
            agreement(&survey, &anchor, member, report);
        }
    }
    survey.relations(report).await?;
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

struct Survey<'a> {
    estate: &'a Estate,
}

impl Survey<'_> {
    fn seat(&self, anchor: &Anchor, held: &[&IssueWorktree], report: &mut Agreement) -> Result<()> {
        let root = self.estate.space.join(".issues").join(&anchor.node);
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
            if name != "worktree" && name != "artifacts" {
                report.fault(
                    "territory.unknown",
                    entry.path().display().to_string(),
                    "undeclared entry occupies the derived Issue seat",
                );
            }
        }
        member(&root.join("worktree"), held, report)?;
        artifacts(&root.join("artifacts"), report)
    }

    async fn relations(&self, report: &mut Agreement) -> Result<()> {
        let members = self.estate.core.live("IssueMember").await.map_err(fault)?;
        for (unit, relation) in [
            ("IssueClaim", "member"),
            ("IssueBoundary", "member"),
            ("IssueChange", "member"),
        ] {
            for row in self.estate.core.live(unit).await.map_err(fault)? {
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

    fn valid(&self, member: &IssueWorktree, path: &Path) -> bool {
        if member.integration.repository.owner != member.issue.owner
            || member.integration.repository.name != member.issue.repository
        {
            return false;
        }
        let Ok(source) = self.estate.issue_source(member) else {
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

    async fn integrations(&self, members: &[IssueWorktree], report: &mut Agreement) -> Result<()> {
        for integration in self.estate.integrations().await? {
            assess(self.estate, members, report, &integration)?;
        }
        Ok(())
    }
}

fn member(root: &Path, held: &[&IssueWorktree], report: &mut Agreement) -> Result<()> {
    if root.exists() && held.is_empty() {
        report.fault(
            "territory.unknown",
            root.display().to_string(),
            "undeclared entry occupies the Issue Member seat",
        );
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

fn agreement(survey: &Survey<'_>, anchor: &Anchor, member: &IssueWorktree, report: &mut Agreement) {
    let subject = anchor.coordinate.identity();
    let path = survey
        .estate
        .space
        .join(".issues")
        .join(&anchor.node)
        .join("worktree");
    if !survey.valid(member, &path) {
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
        && let Some(reason) = stale(proof, member, &path)
    {
        report.observe("boundary.stale", subject, reason);
    }
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
                    left.issue.identity(),
                    format!(
                        "write Claim overlaps {} at {}",
                        right.issue.identity(),
                        paths.join(", ")
                    ),
                );
            }
        }
    }
    Ok(())
}

fn assess(
    state: &Estate,
    members: &[IssueWorktree],
    report: &mut Agreement,
    integration: &super::super::work::Integration,
) -> Result<()> {
    let subject = integration.repository.identity();
    let path = PathBuf::from(&integration.path);
    let observed = match super::super::work::integration::registration::observe(&path) {
        Ok(observed) => observed,
        Err(error) => {
            report.fault("integration.agreement", subject, error.to_string());
            return Ok(());
        }
    };
    if observed.checkout.common.display().to_string() != integration.common
        || integration.remote != "origin"
        || integration.branch != "main"
    {
        report.fault(
            "integration.agreement",
            integration.repository.identity(),
            "Integration path, Git identity, remote, or branch disagrees",
        );
    }
    let mut expected = BTreeSet::from([observed.checkout.path.clone()]);
    for member in members
        .iter()
        .filter(|member| member.integration.key == integration.key)
    {
        expected.insert(
            state
                .space
                .join(".issues")
                .join(&member.node)
                .join("worktree"),
        );
    }
    let mut found = BTreeSet::new();
    for worktree in &observed.worktrees {
        let Some(path) = worktree.path.canonicalize().ok() else {
            continue;
        };
        found.insert(path.clone());
        if !expected.contains(&path) {
            report.fault(
                "integration.worktree.unknown",
                path.display().to_string(),
                format!(
                    "unmanaged worktree belongs to {subject} at head {}",
                    worktree.head.as_deref().unwrap_or("unknown")
                ),
            );
        }
    }
    for path in expected.difference(&found) {
        report.fault(
            "integration.worktree.missing",
            path.display().to_string(),
            format!("declared worktree is not registered for {subject}"),
        );
    }
    Ok(())
}

fn stale(proof: &Proof, member: &IssueWorktree, path: &Path) -> Option<String> {
    let claim = crate::claim::digest(&member.claims);
    match git::at(path).head() {
        Ok(head) => proof.stale(&head, &claim),
        Err(_) => Some("Member Boundary proof is stale: Member HEAD cannot be read".to_owned()),
    }
}
