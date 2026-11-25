use super::{Reply, Server, preflight, reply, success};
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

fn body(heading: &str, outcome: &str) -> String {
    format!(
        "## Problem\nContext\n## Evidence\nObserved\n## {heading}\n{outcome}\n## Scope\nBounded\n## Acceptance\n- [ ] Future check\n## Non-goals\nNo authority\n"
    )
}

fn observe(kind: &str, body: &str) -> Value {
    let fixture = tempfile::tempdir().unwrap();
    success(fixture.path(), None, &["issue", "bootstrap"]);
    let mut page = reply(false);
    page["data"]["repository"]["issue"]["issueType"]["name"] = json!(kind);
    page["data"]["repository"]["issue"]["body"] = json!(body);
    let calls = Arc::new(AtomicUsize::new(0));
    let count = Arc::clone(&calls);
    let server = Server::start(move |_| {
        count.fetch_add(1, Ordering::SeqCst);
        Reply::json(page.clone())
    });
    let report = preflight(fixture.path(), &server, 20);
    assert_eq!(report["preflight"]["complete"], true);
    assert_eq!(report["preflight"]["execution"]["members"], json!([]));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    report["preflight"].clone()
}

fn warning(report: &Value) -> Vec<&Value> {
    report["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|entry| entry["code"] == "concord.issue.sections")
        .collect()
}

struct Case {
    kind: &'static str,
    heading: &'static str,
    outcome: &'static str,
    missing: bool,
}

#[test]
fn types() {
    for case in [
        Case {
            kind: "Bug",
            heading: "Outcome",
            outcome: "Result",
            missing: true,
        },
        Case {
            kind: "Bug",
            heading: "Expected outcome",
            outcome: "Result",
            missing: false,
        },
        Case {
            kind: "Bug",
            heading: "Expected outcome",
            outcome: "  ",
            missing: true,
        },
        Case {
            kind: "Feature",
            heading: "Outcome",
            outcome: "Result",
            missing: false,
        },
        Case {
            kind: "Task",
            heading: "Outcome",
            outcome: "Result",
            missing: false,
        },
        Case {
            kind: "Auto",
            heading: "Outcome",
            outcome: "Result",
            missing: false,
        },
        Case {
            kind: "Ghost",
            heading: "Outcome",
            outcome: "Result",
            missing: false,
        },
    ] {
        let report = observe(case.kind, &body(case.heading, case.outcome));
        let warning = warning(&report);
        assert_eq!(
            warning.len(),
            usize::from(case.missing),
            "{}/{}/{}",
            case.kind,
            case.heading,
            case.outcome
        );
        if case.missing {
            let message = warning[0]["message"].as_str().unwrap();
            assert!(message.contains("Bug") && message.contains("## expected outcome"));
            assert!(!message.contains("unsettled"));
        }
    }
    for (kind, section) in [("Feature", "Problem"), ("Task", "Scope")] {
        let report = observe(
            kind,
            &body("Outcome", "Result").replace(&format!("## {section}\n"), ""),
        );
        let warnings = warning(&report);
        assert_eq!(warnings.len(), 1);
        assert!(
            warnings[0]["message"]
                .as_str()
                .unwrap()
                .contains(&section.to_lowercase())
        );
    }
}

#[test]
fn markdown() {
    for body in [
        body("Outcome", "```\n## Expected outcome\nResult\n```"),
        body("Outcome", "> ## Expected outcome\n> Result"),
        body("Expected outcome", "Result") + "\n## Expected outcome\nDuplicate\n",
    ] {
        let report = observe("Bug", &body);
        let warnings = warning(&report);
        assert_eq!(warnings.len(), 1);
        assert!(
            warnings[0]["message"]
                .as_str()
                .unwrap()
                .contains("## expected outcome")
        );
    }
}

fn form(heading: &str, outcome: &str) -> String {
    format!(
        "### Problem\nContext\n### Evidence\nObserved\n### {heading}\n{outcome}\n### Scope\nBounded\n### Acceptance\n- [ ] Future check\n### Non-goals\nNo authority\n"
    )
}

#[test]
fn forms() {
    for case in [
        Case {
            kind: "Task",
            heading: "Outcome",
            outcome: "Result",
            missing: false,
        },
        Case {
            kind: "Feature",
            heading: "Outcome",
            outcome: "Result",
            missing: false,
        },
        Case {
            kind: "Bug",
            heading: "Expected outcome",
            outcome: "Result",
            missing: false,
        },
        Case {
            kind: "Bug",
            heading: "Outcome",
            outcome: "Result",
            missing: true,
        },
        Case {
            kind: "Bug",
            heading: "Expected outcome",
            outcome: "  ",
            missing: true,
        },
    ] {
        let report = observe(case.kind, &form(case.heading, case.outcome));
        let warnings = warning(&report);
        assert_eq!(
            warnings.len(),
            usize::from(case.missing),
            "{}/{}/{}",
            case.kind,
            case.heading,
            case.outcome
        );
    }
    for body in [
        form("Outcome", "```\n### Expected outcome\nResult\n```"),
        form("Outcome", "> ### Expected outcome\n> Result"),
        form("Expected outcome", "Result") + "\n### Expected outcome\nDuplicate\n",
    ] {
        let report = observe("Bug", &body);
        assert_eq!(warning(&report).len(), 1);
    }
    let report = observe(
        "Task",
        &form("Outcome", "Result").replace("### Scope\n", ""),
    );
    assert_eq!(warning(&report).len(), 1);
}
