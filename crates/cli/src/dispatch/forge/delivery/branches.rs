use super::super::github::issues::{Observed, states};
use super::git::Git;
use super::pull::provider;
use concord_core::{Error, Estate, Integration, Result};
use serde_json::json;
use std::collections::BTreeSet;
use std::path::Path;

const BOUND: usize = 100;
const KINDS: [(&str, &str); 3] = [("feature", "Feature"), ("bug", "Bug"), ("task", "Task")];

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Reason {
    Name,
    Delivery,
    Absent,
    Closed,
    Kind,
}

#[derive(Debug, Eq, Ord, PartialEq, PartialOrd)]
struct Offense {
    branch: String,
    place: &'static str,
    reason: Reason,
}

struct Pending {
    branch: String,
    place: &'static str,
    kind: &'static str,
    number: u64,
}

#[derive(Debug, Eq, PartialEq)]
enum Verdict {
    Held,
    Issue { kind: &'static str, number: u64 },
    Refused(Reason),
}

pub(super) async fn clean(
    estate: &Estate,
    integration: &Integration,
    command: &Path,
    timeout: u64,
) -> Result<()> {
    let live = estate
        .issue_worktrees()
        .await?
        .into_iter()
        .filter(|member| member.integration.node == integration.node)
        .flat_map(|member| [format!("land/{}", member.branch), member.branch])
        .collect::<BTreeSet<_>>();
    let held = listed(Path::new(&integration.path), &integration.remote).await?;
    let mut offenses = Vec::new();
    let mut pending = Vec::new();
    for (branch, place) in held {
        match judge(&branch, &integration.branch, &live) {
            Verdict::Held => {}
            Verdict::Refused(reason) => offenses.push(Offense {
                branch,
                place,
                reason,
            }),
            Verdict::Issue { kind, number } => pending.push(Pending {
                branch,
                place,
                kind,
                number,
            }),
        }
    }
    let numbers = pending
        .iter()
        .map(|held| held.number)
        .collect::<BTreeSet<_>>();
    if numbers.len() > BOUND {
        return Err(Error::typed(
            "concord.delivery.branches",
            format!(
                "{} names {} Issue branches, more than the {BOUND} one bounded query reads; delete stale branches before delivering",
                identity(integration),
                numbers.len()
            ),
        ));
    }
    let observed = states(&integration.repository, &numbers, command, timeout).await?;
    for held in pending {
        if let Some(reason) = settle(held.kind, observed.get(&held.number)) {
            offenses.push(Offense {
                branch: held.branch,
                place: held.place,
                reason,
            });
        }
    }
    refuse(integration, offenses)
}

fn judge(branch: &str, main: &str, live: &BTreeSet<String>) -> Verdict {
    if concord_core::automation::branch(branch).is_some() {
        return Verdict::Held;
    }
    if branch == main || live.contains(branch) || release(branch) {
        return Verdict::Held;
    }
    if branch.starts_with("land/") {
        return Verdict::Refused(Reason::Delivery);
    }
    let Some((prefix, number)) = branch.split_once('/') else {
        return Verdict::Refused(Reason::Name);
    };
    let kind = KINDS.iter().find(|(held, _)| *held == prefix);
    match (kind, canonical(number).filter(|number| *number > 0)) {
        (Some((_, kind)), Some(number)) => Verdict::Issue { kind, number },
        _ => Verdict::Refused(Reason::Name),
    }
}

fn release(branch: &str) -> bool {
    branch
        .strip_prefix("release/v")
        .map(|version| version.split('.').collect::<Vec<_>>())
        .is_some_and(|parts| parts.len() == 3 && parts.iter().all(|part| canonical(part).is_some()))
}

fn canonical(text: &str) -> Option<u64> {
    text.parse::<u64>()
        .ok()
        .filter(|number| number.to_string() == text)
}

fn settle(kind: &str, observed: Option<&Observed>) -> Option<Reason> {
    match observed {
        None | Some(Observed::Absent) => Some(Reason::Absent),
        Some(Observed::Issue { open: false, .. }) => Some(Reason::Closed),
        Some(Observed::Issue { kind: held, .. }) if held.as_deref() != Some(kind) => {
            Some(Reason::Kind)
        }
        Some(Observed::Issue { .. }) => None,
    }
}

async fn listed(root: &Path, remote: &str) -> Result<Vec<(String, &'static str)>> {
    let local = Git::run(
        root,
        &["for-each-ref", "--format=%(refname:strip=2)", "refs/heads"],
    )
    .await?;
    let mut held = local
        .lines()
        .map(|branch| (branch.to_string(), "local"))
        .collect::<Vec<_>>();
    for line in Git::run(root, &["ls-remote", "--heads", remote])
        .await?
        .lines()
    {
        let branch = line
            .split_once('\t')
            .and_then(|(_, reference)| reference.strip_prefix("refs/heads/"))
            .ok_or_else(|| provider(format!("git ls-remote answered a malformed line: {line}")))?;
        held.push((branch.to_string(), "origin"));
    }
    Ok(held)
}

fn refuse(integration: &Integration, mut offenses: Vec<Offense>) -> Result<()> {
    if offenses.is_empty() {
        return Ok(());
    }
    offenses.sort();
    let listed = offenses
        .iter()
        .map(|offense| {
            format!(
                "{} {} ({})",
                offense.place,
                offense.branch,
                describe(offense.reason)[1]
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    let details = offenses
        .iter()
        .map(|offense| {
            json!({
                "branch": offense.branch,
                "place": offense.place,
                "reason": describe(offense.reason)[0],
            })
        })
        .collect::<Vec<_>>();
    Err(Error::detailed(
        "concord.delivery.branches",
        format!(
            "{} holds branches outside the Issue branch set: {listed}; delete them, or give work worth keeping an open Issue and rename it to <type>/<number>, before delivering",
            identity(integration)
        ),
        json!({"repository": identity(integration), "branches": details}),
    ))
}

fn identity(integration: &Integration) -> String {
    format!(
        "{}/{}",
        integration.repository.owner, integration.repository.name
    )
}

fn describe(reason: Reason) -> [&'static str; 2] {
    match reason {
        Reason::Name => ["name", "name"],
        Reason::Delivery => ["delivery", "no live delivery"],
        Reason::Absent => ["absent", "no such Issue"],
        Reason::Closed => ["closed", "closed Issue"],
        Reason::Kind => ["type", "Issue type differs"],
    }
}

#[cfg(test)]
mod tests {
    use super::{Observed, Reason, Verdict};
    use std::collections::BTreeSet;

    #[test]
    fn judge() {
        let live = BTreeSet::from([
            "concord/issue-abc".to_string(),
            "land/concord/issue-abc".to_string(),
        ]);
        for branch in [
            "main",
            "release/v0.22.0",
            "concord/issue-abc",
            "land/concord/issue-abc",
        ] {
            assert_eq!(
                super::judge(branch, "main", &live),
                Verdict::Held,
                "{branch}"
            );
        }
        assert_eq!(
            super::judge("task/12", "main", &live),
            Verdict::Issue {
                kind: "Task",
                number: 12
            }
        );
        for branch in [
            "topic",
            "fix/12",
            "task/012",
            "task/0",
            "task/12/more",
            "release/v1.2",
            "release/v01.2.3",
            "concord/issue-def",
        ] {
            assert_eq!(
                super::judge(branch, "main", &live),
                Verdict::Refused(Reason::Name),
                "{branch}"
            );
        }
        assert_eq!(
            super::judge("land/task/12", "main", &live),
            Verdict::Refused(Reason::Delivery)
        );
    }

    #[test]
    fn settle() {
        let open = |kind: &str| Observed::Issue {
            open: true,
            kind: Some(kind.to_string()),
        };
        assert_eq!(super::settle("Task", Some(&open("Task"))), None);
        assert_eq!(
            super::settle("Task", Some(&open("Bug"))),
            Some(Reason::Kind)
        );
        assert_eq!(
            super::settle(
                "Task",
                Some(&Observed::Issue {
                    open: false,
                    kind: Some("Task".into())
                })
            ),
            Some(Reason::Closed)
        );
        assert_eq!(
            super::settle("Task", Some(&Observed::Absent)),
            Some(Reason::Absent)
        );
    }
}
