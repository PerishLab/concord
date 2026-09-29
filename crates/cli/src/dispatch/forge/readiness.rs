use super::projection::{self, Connection, Projection, Readiness, ReadinessChecks};
use concord_core::{Coordinate, Result};
use std::collections::{BTreeMap, BTreeSet};
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
                    Connection::Comments,
                ]),
            )
            .await
            .map_err(|error| {
                projection::record("issue.ready", started.elapsed(), error.code);
                projection::fault(error)
            })?;
        let sections = sections(&held.body);
        let missing = required_sections(&held.kind)
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
        let sub_issues_closed = held.sub_issues.iter().all(|issue| issue.state == "closed");
        let blockers_closed = held.blocked_by.iter().all(|issue| issue.state == "closed");
        let checks = ReadinessChecks {
            required_sections: missing.is_empty(),
            acceptance_nonempty: total > 0,
            acceptance_settled: total > 0 && open == 0,
            acceptance_total: total,
            acceptance_open: open,
            sub_issues_closed,
            blockers_closed,
        };
        let reasons = reasons(&missing, &checks);
        let mut evidence = BTreeSet::new();
        distribution_evidence(&held.body, &mut evidence);
        for comment in &held.comments {
            distribution_evidence(&comment.body, &mut evidence);
        }
        let readiness = Readiness {
            schema: projection::READINESS,
            issue: held.issue,
            kind: held.kind,
            ready: reasons.is_empty(),
            checks,
            reasons,
            distribution_evidence: evidence.into_iter().collect(),
            observed_at: projection::now(),
        };
        projection::record("issue.ready", started.elapsed(), "fresh");
        Ok(readiness)
    }
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

fn sections(body: &str) -> BTreeMap<String, String> {
    let mut result = BTreeMap::new();
    let mut current: Option<(String, usize)> = None;
    let mut offset = 0;
    for line in body.split_inclusive('\n') {
        if let Some(name) = line.trim_end().strip_prefix("## ") {
            if let Some((name, start)) = current.take() {
                result.insert(name, body[start..offset].trim().to_string());
            }
            current = Some((name.trim().to_ascii_lowercase(), offset + line.len()));
        }
        offset += line.len();
    }
    if let Some((name, start)) = current {
        result.insert(name, body[start..].trim().to_string());
    }
    result
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

fn checkboxes(section: &str) -> (usize, usize) {
    let mut total = 0;
    let mut open = 0;
    for line in section.lines().map(str::trim) {
        let item = line
            .strip_prefix("- ")
            .or_else(|| line.strip_prefix("* "))
            .unwrap_or(line);
        if item.starts_with("[ ] ") {
            total += 1;
            open += 1;
        } else if item.starts_with("[x] ") || item.starts_with("[X] ") {
            total += 1;
        }
    }
    (total, open)
}

fn distribution_evidence(body: &str, evidence: &mut BTreeSet<String>) {
    for token in body.split_whitespace() {
        let token = token.trim_matches(|character: char| {
            matches!(
                character,
                '(' | ')' | '[' | ']' | '<' | '>' | ',' | '.' | ';' | '"' | '\''
            )
        });
        if token.starts_with("https://releases.plumb.perish.uk/")
            && token.contains("/distribution.json")
        {
            evidence.insert(token.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{checkboxes, distribution_evidence, outcome, sections};
    use std::collections::BTreeSet;

    #[test]
    fn evidence() {
        let body =
            "## Outcome\nDone\n\n## Acceptance\n- [x] one\n- [ ] two\n\n## Non-goals\nNone\n";
        let held = sections(body);
        assert_eq!(held["outcome"], "Done");
        assert_eq!(checkboxes(&held["acceptance"]), (2, 1));
        let mut evidence = BTreeSet::new();
        distribution_evidence(
            "proof: https://releases.plumb.perish.uk/v1/releases/stable/v1/distribution.json",
            &mut evidence,
        );
        assert_eq!(evidence.len(), 1);
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
