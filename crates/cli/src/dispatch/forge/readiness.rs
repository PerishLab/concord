use super::projection::authority::{evidence, trust};
use super::projection::{self, Connection, Fault, Projection, Readiness, ReadinessChecks};
use concord_core::acceptance::{Verdict, checkboxes, sections};
use concord_core::{Coordinate, Error, Result};
use std::collections::BTreeSet;
use std::time::Instant;

impl Projection<'_> {
    pub async fn ready(&self, coordinate: &Coordinate) -> Result<Readiness> {
        let started = Instant::now();
        let held = self
            .complete(
                coordinate,
                BTreeSet::from([
                    Connection::SubIssues,
                    Connection::BlockedBy,
                    Connection::Pulls,
                    Connection::Comments,
                ]),
            )
            .await
            .map_err(|error| refuse(started, error))?;
        let authorities = self
            .authorities(&trust(&held.issue.coordinate, &held.pulls))
            .await
            .map_err(|error| refuse(started, error))?;
        let texts = held.comments.iter().map(|comment| comment.body.as_str());
        let distribution = evidence(texts.chain([held.body.as_str()]), &authorities);
        let args = crate::args::acceptance::Observe {
            issue: format!(
                "{}/{}#{}",
                coordinate.owner, coordinate.repository, coordinate.number
            ),
            command: self.command.to_path_buf(),
            timeout: self.timeout,
            pages: usize::from(self.max_pages),
        };
        let reviewed = crate::dispatch::acceptance::review(&args).await?;
        if !reviewed.current(&held.issue.node, &held.kind, &held.body) {
            return Err(Error::typed(
                "concord.acceptance.changed",
                "readiness and acceptance Issue basis differ",
            ));
        }
        let (checks, mut reasons) = conditions(
            &held.body,
            &held.kind,
            reviewed.settled("children"),
            reviewed.settled("blockers"),
        );
        let accepted = reviewed.accepted();
        if !accepted {
            reasons.extend(
                reviewed
                    .evaluation
                    .checks
                    .iter()
                    .filter(|check| check.fact.verdict != Verdict::Satisfied)
                    .map(|check| {
                        format!(
                            "{}: {:?}: {}",
                            check.name, check.fact.verdict, check.fact.reason
                        )
                    }),
            );
        }
        let readiness = Readiness {
            schema: projection::READINESS,
            issue: held.issue,
            kind: held.kind,
            ready: reasons.is_empty() && accepted,
            checks,
            acceptance: reviewed.evaluation,
            reasons,
            distribution_evidence: distribution,
            observed_at: projection::now(),
        };
        projection::record("issue.ready", started.elapsed(), "fresh");
        Ok(readiness)
    }
}

pub(in crate::dispatch) fn conditions(
    body: &str,
    kind: &str,
    children: bool,
    blockers: bool,
) -> (ReadinessChecks, Vec<String>) {
    let sections = sections(body);
    let missing = required_sections(kind)
        .iter()
        .filter(|name| {
            sections
                .get(**name)
                .is_none_or(|body| body.trim().is_empty())
        })
        .copied()
        .collect::<Vec<_>>();
    let acceptance = sections
        .get("acceptance")
        .map(String::as_str)
        .unwrap_or_default();
    let (total, open) = checkboxes(acceptance);
    let checks = ReadinessChecks {
        required_sections: missing.is_empty(),
        acceptance_nonempty: total > 0,
        acceptance_settled: total > 0 && open == 0,
        acceptance_total: total,
        acceptance_open: open,
        sub_issues_closed: children,
        blockers_closed: blockers,
    };
    let reasons = reasons(&missing, &checks);
    (checks, reasons)
}

fn refuse(started: Instant, error: Fault) -> Error {
    projection::record("issue.ready", started.elapsed(), error.code);
    projection::fault(error)
}

fn reasons(missing: &[&str], checks: &ReadinessChecks) -> Vec<String> {
    let mut reasons = Vec::new();
    if !missing.is_empty() {
        reasons.push(format!("missing required sections: {}", missing.join(", ")));
    }
    if !checks.acceptance_nonempty {
        reasons.push("Acceptance has no checklist items".to_string());
    } else if !checks.acceptance_settled {
        reasons.push(format!(
            "Acceptance has {} unsettled item(s)",
            checks.acceptance_open
        ));
    }
    if !checks.sub_issues_closed {
        reasons.push("one or more sub-issues are open".to_string());
    }
    if !checks.blockers_closed {
        reasons.push("one or more blockers are open".to_string());
    }
    reasons
}

pub(super) fn outcome(body: &str, kind: &str) -> Option<String> {
    let name = match kind.to_ascii_lowercase().as_str() {
        "bug" => "expected outcome",
        _ => "outcome",
    };
    sections(body)
        .remove(name)
        .filter(|outcome| !outcome.trim().is_empty())
}

fn required_sections(kind: &str) -> &'static [&'static str] {
    match kind.to_ascii_lowercase().as_str() {
        "feature" => &["problem", "outcome", "acceptance", "non-goals"],
        "bug" => &[
            "problem",
            "evidence",
            "expected outcome",
            "acceptance",
            "non-goals",
        ],
        "task" => &["outcome", "scope", "acceptance", "non-goals"],
        _ => &["outcome", "acceptance", "non-goals"],
    }
}

#[cfg(test)]
mod tests {
    use super::{checkboxes, outcome, sections};

    #[test]
    fn evidence() {
        let body =
            "## Outcome\nDone\n\n## Acceptance\n- [x] one\n- [ ] two\n\n## Non-goals\nNone\n";
        let held = sections(body);
        assert_eq!(held["outcome"], "Done");
        assert_eq!(checkboxes(&held["acceptance"]), (2, 1));
    }

    #[test]
    fn kinds() {
        let task = "## Outcome\nDone\n\n## Scope\nAll\n";
        let bug = "## Problem\nBroken\n\n## Expected outcome\nFixed\n";
        assert_eq!(outcome(task, "Task").as_deref(), Some("Done"));
        assert_eq!(outcome(task, "Feature").as_deref(), Some("Done"));
        assert_eq!(outcome(bug, "Bug").as_deref(), Some("Fixed"));
        assert_eq!(outcome(task, "Bug"), None);
        assert_eq!(outcome(bug, "Task"), None);
    }
}
