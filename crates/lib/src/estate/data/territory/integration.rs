use super::super::work::Integration;
use super::super::{Agreement, Estate, IssueWorktree, Scope};
use crate::Result;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[locus::trace(with = crate::observation::view().as_ref().map(crate::observation::borrow), frame = frame)]
pub(super) async fn survey(
    estate: &Estate,
    members: &[IssueWorktree],
    report: &mut Agreement,
    scope: Scope<'_>,
) -> Result<()> {
    let frame = frame.as_ref();
    let space = &estate.layout.root;
    let integrations = estate
        .integrations()
        .await?
        .into_iter()
        .filter(|integration| scope.integration(integration))
        .collect::<Vec<_>>();
    let parts = std::thread::scope(|threads| {
        let tasks = integrations
            .iter()
            .map(|integration| {
                let context = crate::observation::fork(frame);
                let run = move || assess(space, members, integration, scope);
                threads.spawn(move || crate::observation::within(context, run))
            })
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

fn assess(
    space: &Path,
    members: &[IssueWorktree],
    integration: &Integration,
    scope: Scope<'_>,
) -> Result<Agreement> {
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
        expected.insert(space.join("issues").join(&member.node).join("worktree"));
    }
    let required = members
        .iter()
        .filter(|member| member.integration.key == integration.key && scope.member(member))
        .map(|member| space.join("issues").join(&member.node).join("worktree"))
        .chain([observed.checkout.path])
        .collect::<BTreeSet<_>>();
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
            let occupied = path.starts_with(space)
                || space.starts_with(&path)
                || required
                    .iter()
                    .any(|owned| path.starts_with(owned) || owned.starts_with(&path));
            let conflict = occupied
                || worktree.branch.as_deref() == Some(integration.branch.as_str())
                || members
                    .iter()
                    .filter(|member| scope.member(member))
                    .any(|member| {
                        member.integration.key == integration.key
                            && worktree.branch.as_deref() == Some(member.branch.as_str())
                    });
            let message = format!(
                "unmanaged worktree belongs to {subject} at head {}",
                worktree.head.as_deref().unwrap_or("unknown")
            );
            if scope.complete() || conflict {
                report.fault(
                    "integration.worktree.unknown",
                    path.display().to_string(),
                    message,
                );
            } else {
                report.observe(
                    "integration.worktree.unknown",
                    path.display().to_string(),
                    message,
                );
            }
        }
    }
    for path in required.difference(&found) {
        report.fault(
            "integration.worktree.missing",
            path.display().to_string(),
            format!("declared worktree is not registered for {subject}"),
        );
    }
    Ok(report)
}
