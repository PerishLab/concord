use super::super::work::Integration;
use super::super::{Agreement, IssueWorktree};
use crate::Result;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub(super) fn survey(
    space: &Path,
    members: &[IssueWorktree],
    integrations: &[Integration],
    report: &mut Agreement,
) -> Result<()> {
    let parts = std::thread::scope(|scope| {
        let tasks = integrations
            .iter()
            .map(|integration| scope.spawn(move || assess(space, members, integration)))
            .collect::<Vec<_>>();
        tasks
            .into_iter()
            .map(|task| {
                task.join()
                    .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
            })
            .collect::<Vec<_>>()
    });
    for part in parts {
        let found = part?;
        report.faults.extend(found.faults);
        report.observations.extend(found.observations);
    }
    Ok(())
}

fn assess(space: &Path, members: &[IssueWorktree], integration: &Integration) -> Result<Agreement> {
    let mut report = Agreement {
        target: String::new(),
        revision: 0,
        faults: Vec::new(),
        observations: Vec::new(),
    };
    let subject = integration.repository.identity();
    let path = PathBuf::from(&integration.path);
    let observed = match super::super::work::integration::registration::observe(&path) {
        Ok(observed) => observed,
        Err(error) => {
            let message = if !path.exists() {
                format!(
                    "{error}; if the operator intentionally removed this idle repository, preflight concord integration retire {subject} --node {} --key {}; apply only after reviewing that exact registration",
                    integration.node, integration.key,
                )
            } else {
                error.to_string()
            };
            report.fault("integration.agreement", subject, message);
            return Ok(report);
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
        expected.insert(space.join(".issues").join(&member.node).join("worktree"));
    }
    let mut found = BTreeSet::new();
    for worktree in observed.worktrees.iter().filter(|worktree| {
        !worktree.staging
            && worktree
                .branch
                .as_deref()
                .and_then(crate::automation::branch)
                .is_none()
    }) {
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
    Ok(report)
}
