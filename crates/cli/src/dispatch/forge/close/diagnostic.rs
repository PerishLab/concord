use concord_core::Error;
use concord_core::acceptance::{Check, Evaluation, Verdict};
use serde::Serialize;
use serde_json::json;

const LIMIT: usize = 8;
const BYTES: usize = 2048;

#[derive(Serialize)]
struct Entry {
    name: &'static str,
    verdict: Verdict,
    reason: &'static str,
}

#[derive(Serialize)]
struct Summary {
    verdict: Verdict,
    checks: Vec<Entry>,
    truncated: bool,
}

pub(super) fn refused(evaluation: &Evaluation) -> Error {
    let mut summary = Summary {
        verdict: evaluation.verdict,
        checks: Vec::new(),
        truncated: false,
    };
    for check in &evaluation.checks {
        if check.fact.verdict == Verdict::Satisfied {
            continue;
        }
        if summary.checks.len() == LIMIT {
            summary.truncated = true;
            break;
        }
        summary.checks.push(Entry {
            name: name(&check.name),
            verdict: check.fact.verdict,
            reason: reason(check),
        });
        if json!(&summary).to_string().len() > BYTES {
            summary.checks.pop();
            summary.truncated = true;
            break;
        }
    }
    let checks = summary
        .checks
        .iter()
        .map(|check| format!("{} ({}): {}", check.name, word(check.verdict), check.reason))
        .collect::<Vec<_>>()
        .join("; ");
    let suffix = if summary.truncated {
        "; further acceptance checks omitted"
    } else {
        ""
    };
    Error::detailed(
        "concord.issue.close.acceptance",
        format!(
            "current adopted acceptance is not satisfied; inspect acceptance evaluate and issue ready; acceptance {}: {checks}{suffix}",
            word(summary.verdict),
        ),
        json!(summary),
    )
}

fn word(verdict: Verdict) -> &'static str {
    match verdict {
        Verdict::Satisfied => "satisfied",
        Verdict::Unmet => "unmet",
        Verdict::Unknown => "unknown",
    }
}

fn name(name: &str) -> &'static str {
    match name {
        "closure" => "closure",
        "judgment" => "judgment",
        "review" => "review",
        "freshness" => "freshness",
        "sections" => "sections",
        "checklist" => "checklist",
        "children" => "children",
        "blockers" => "blockers",
        "conditions" => "conditions",
        "release" => "release",
        _ => "other",
    }
}

fn reason(check: &Check) -> &'static str {
    match name(&check.name) {
        "closure" => "current closure judgment is unavailable",
        "judgment" => "closure judgment is not satisfied",
        "review" => "closure review does not establish current body agreement",
        "freshness" => freshness(&check.fact.reason),
        "sections" => "required Issue sections are not satisfied",
        "checklist" => "acceptance checklist is not settled",
        "children" => "native sub-issues are not settled",
        "blockers" => "native blockers are not settled",
        "conditions" => "detailed acceptance facts were not observed",
        "release" => release(&check.fact.reason),
        _ => "unrecognized acceptance check; original name and reason withheld",
    }
}

fn freshness(reason: &str) -> &'static str {
    match reason {
        "closure comment was edited; publish a fresh review" => {
            "closure comment was edited; publish a fresh review"
        }
        "body edit, declaration edit or reopen followed the closure review" => {
            "body edit, declaration edit or reopen followed the closure review"
        }
        "provider timestamps cannot order a change and review within the same second" => {
            "provider timestamps cannot order a change and review within the same second"
        }
        "provider timestamps do not establish declaration before review" => {
            "provider timestamps do not establish declaration before review"
        }
        "no current review to bind freshness" => "no current review to bind freshness",
        _ => "review freshness is not established; unrecognized reason withheld",
    }
}

fn release(reason: &str) -> &'static str {
    match reason {
        "no current release review" => "no current release review",
        "repository declares no release authority" => "repository declares no release authority",
        "distribution is outside the repository release authority" => {
            "distribution is outside the repository release authority"
        }
        "release review does not name one exact stable distribution" => {
            "release review does not name one exact stable distribution"
        }
        "exact distribution record is absent" => "exact distribution record is absent",
        "distribution marker differs from release review" => {
            "distribution marker differs from release review"
        }
        "exact stable distribution is not complete" => "exact stable distribution is not complete",
        "complete distribution lacks a source commit or public media" => {
            "complete distribution lacks a source commit or public media"
        }
        "distribution alone does not prove inclusion; machine inclusion evidence is not available" => {
            "distribution alone does not prove inclusion; machine inclusion evidence is not available"
        }
        value if value.starts_with("distribution observation failed: ") => {
            "distribution observation failed; provider details withheld"
        }
        _ => "release condition is not established; unrecognized reason withheld",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use concord_core::acceptance::Fact;

    fn evaluation(names: &[&str]) -> Evaluation {
        Evaluation {
            target: None,
            declaration: None,
            closure: None,
            verification: None,
            verdict: Verdict::Unknown,
            checks: names
                .iter()
                .map(|name| Check {
                    name: (*name).into(),
                    fact: Fact {
                        verdict: Verdict::Unknown,
                        reason: "TOP_SECRET https://secret.invalid/?token=TOP_SECRET I_private"
                            .repeat(4096),
                    },
                })
                .collect(),
        }
    }

    #[test]
    fn redacted() {
        for name in [
            "closure",
            "judgment",
            "review",
            "freshness",
            "sections",
            "checklist",
            "children",
            "blockers",
            "conditions",
            "release",
            "TOP_SECRET",
        ] {
            let observed = evaluation(&[name]);
            let failure = refused(&observed);
            let details = failure.details().unwrap().clone();
            assert_eq!(details["checks"][0]["name"], super::name(name));
            assert_eq!(details["checks"][0]["verdict"], "unknown");
            let rendered = format!("{failure:?} {failure}");
            for secret in ["TOP_SECRET", "secret.invalid", "I_private"] {
                assert!(!rendered.contains(secret), "{name}");
            }
            let wrapped = super::super::unknown(failure, "acceptance");
            assert_eq!(wrapped.details().unwrap()["cause"], details);
        }
    }

    #[test]
    fn bounds() {
        let names = [
            "closure",
            "judgment",
            "review",
            "freshness",
            "sections",
            "checklist",
            "children",
            "blockers",
            "conditions",
            "release",
            "TOP_SECRET",
        ];
        for count in 0..=names.len() {
            let failure = refused(&evaluation(&names[..count]));
            let details = failure.details().unwrap();
            assert_eq!(
                details["checks"].as_array().unwrap().len(),
                count.min(LIMIT)
            );
            assert_eq!(details["truncated"], count > LIMIT);
            assert!(details.to_string().len() <= BYTES);
            assert!(failure.message().len() <= BYTES);
        }
        let mut observed = evaluation(&names);
        for check in &mut observed.checks {
            check.fact.verdict = Verdict::Satisfied;
        }
        let failure = refused(&observed);
        assert_eq!(failure.details().unwrap()["checks"], json!([]));
        assert_eq!(failure.details().unwrap()["truncated"], false);
    }

    #[test]
    fn reasons() {
        for (name, reason, expected) in [
            (
                "freshness",
                "provider timestamps cannot order a change and review within the same second",
                "provider timestamps cannot order a change and review within the same second",
            ),
            (
                "release",
                "exact distribution record is absent",
                "exact distribution record is absent",
            ),
            (
                "release",
                "distribution observation failed: TOP_SECRET",
                "distribution observation failed; provider details withheld",
            ),
        ] {
            let mut observed = evaluation(&[name]);
            observed.checks[0].fact.reason = reason.into();
            let failure = refused(&observed);
            assert_eq!(failure.details().unwrap()["checks"][0]["reason"], expected);
            assert!(failure.message().contains(expected));
        }
        let error = Error::detailed(
            "concord.acceptance.provider",
            "TOP_SECRET",
            json!({"secret":"TOP_SECRET"}),
        );
        let wrapped = super::super::unknown(error, "confirmation");
        assert!(!format!("{wrapped:?}").contains("TOP_SECRET"));
        assert_eq!(
            wrapped.details().unwrap()["cause_code"],
            "concord.acceptance.provider"
        );
        assert!(wrapped.details().unwrap().get("cause").is_none());
    }
}
