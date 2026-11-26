use super::projection::authority::{evidence, trust};
use super::projection::{self, Checks, Complete, Connection, Fault, Projection, Readiness};
use concord_core::acceptance::{Verdict, checkboxes, sections};
use concord_core::{Coordinate, Error, Result};
use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

impl Projection {
    pub async fn ready(&self, coordinate: &Coordinate) -> Result<Readiness> {
        let started = Instant::now();
        let held = self
            .complete(
                coordinate,
                BTreeSet::from([
                    Connection::Children,
                    Connection::Blockers,
                    Connection::Pulls,
                    Connection::Comments,
                ]),
            )
            .await
            .map_err(|error| refuse(started, error))?;
        if held.kind == "Auto" {
            return Ok(automation(held));
        }
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
            timeout: self.timeout,
            pages: usize::from(self.pages),
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
            acceptance: Some(reviewed.evaluation),
            automation: None,
            reasons,
            evidence: distribution,
            observed: projection::now(),
        };
        projection::record("issue.ready", started.elapsed(), "fresh");
        Ok(readiness)
    }
}

fn automation(held: Complete) -> Readiness {
    let automation = concord_core::automation::held(&held.kind, &held.body, &held.issue.state);
    let children = held.children.iter().all(|issue| issue.state == "closed");
    let blockers = held.blockers.iter().all(|issue| issue.state == "closed");
    let (checks, mut reasons) = conditions(&held.body, &held.kind, children, blockers);
    reasons.push("Auto is automation-held: Plumb owns verification and closure; Concord projects current facts read-only".to_string());
    Readiness {
        schema: projection::READINESS,
        issue: held.issue,
        kind: held.kind,
        ready: false,
        checks,
        acceptance: None,
        automation,
        reasons,
        evidence: Vec::new(),
        observed: projection::now(),
    }
}

pub(in crate::dispatch) fn conditions(
    body: &str,
    kind: &str,
    children: bool,
    blockers: bool,
) -> (Checks, Vec<String>) {
    let sections = sections(body);
    let missing = missing(&sections, kind);
    let acceptance = sections
        .get("acceptance")
        .map(String::as_str)
        .unwrap_or_default();
    let (total, open) = checkboxes(acceptance);
    let checks = Checks {
        sections: missing.is_empty(),
        nonempty: total > 0,
        settled: total > 0 && open == 0,
        total,
        open,
        children,
        blockers,
    };
    let reasons = reasons(&missing, &checks);
    (checks, reasons)
}

fn missing(sections: &BTreeMap<String, String>, kind: &str) -> Vec<&'static str> {
    required(kind)
        .iter()
        .filter(|name| {
            sections
                .get(**name)
                .is_none_or(|body| body.trim().is_empty())
        })
        .copied()
        .collect()
}

pub(super) fn advice(body: &str, kind: &str) -> Option<String> {
    if !matches!(kind, "Feature" | "Bug" | "Task") {
        return None;
    }
    let missing = missing(&sections(body), kind);
    (!missing.is_empty()).then(|| {
        format!(
            "{kind} needs nonempty required sections: {}. Add these sections before starting Member work; see concord issue preflight existing --help",
            missing.iter().map(|name| format!("## {name}")).collect::<Vec<_>>().join(", "),
        )
    })
}

fn refuse(started: Instant, error: Fault) -> Error {
    projection::record("issue.ready", started.elapsed(), error.code);
    projection::fault(error)
}

fn reasons(missing: &[&str], checks: &Checks) -> Vec<String> {
    let mut reasons = Vec::new();
    if !missing.is_empty() {
        reasons.push(format!("missing required sections: {}", missing.join(", ")));
    }
    if !checks.nonempty {
        reasons.push("Acceptance has no checklist items".to_string());
    } else if !checks.settled {
        reasons.push(format!("Acceptance has {} unsettled item(s)", checks.open));
    }
    if !checks.children {
        reasons.push("one or more sub-issues are open".to_string());
    }
    if !checks.blockers {
        reasons.push("one or more blockers are open".to_string());
    }
    reasons
}

pub(super) fn outcome(body: &str, kind: &str) -> Option<String> {
    sections(body)
        .remove(heading(kind))
        .filter(|outcome| !outcome.trim().is_empty())
}

pub(super) fn heading(kind: &str) -> &'static str {
    match kind.to_ascii_lowercase().as_str() {
        "bug" => "expected outcome",
        _ => "outcome",
    }
}

fn required(kind: &str) -> &'static [&'static str] {
    match kind.to_ascii_lowercase().as_str() {
        "auto" => &["operation", "target", "change", "acceptance", "non-goals"],
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
    fn headings() {
        for (kind, heading, other) in [
            ("Bug", "Expected outcome", "Outcome"),
            ("Feature", "Outcome", "Expected outcome"),
            ("Task", "Outcome", "Expected outcome"),
        ] {
            for body in [
                "".to_string(),
                format!("## {heading}\n \n"),
                format!("## {other}\nWrong"),
            ] {
                assert_eq!(outcome(&body, kind), None, "{kind}: {body}");
            }
            assert_eq!(
                outcome(&format!("## {heading}\nFixed"), kind).as_deref(),
                Some("Fixed")
            );
        }
        assert_eq!(outcome("## Expected result\nFixed", "Bug"), None);
        assert_eq!(
            outcome("## Expected outcome\nFixed", "bUg").as_deref(),
            Some("Fixed")
        );
        assert_eq!(outcome("## Expected outcome\nFixed", " Bug "), None);
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
