use super::{preflight, reply, success, tool};
use serde_json::{Value, json};

fn body(heading: &str, outcome: &str) -> String {
    format!(
        "## Problem\nContext\n## Evidence\nObserved\n## {heading}\n{outcome}\n## Scope\nBounded\n## Acceptance\n- [ ] Future check\n## Non-goals\nNo authority\n"
    )
}

fn observe(kind: &str, body: &str) -> Value {
    let fixture = tempfile::tempdir().unwrap();
    success(fixture.path(), &["issue", "bootstrap"]);
    let page = reply(false)
        .replace("\"Task\"", &json!(kind).to_string())
        .replace(
            &json!("## Outcome\nExplain native relationships").to_string(),
            &json!(body).to_string(),
        );
    let path = fixture.path().join("calls");
    let script = format!(
        "count=0\nif [ -f '{0}' ]; then read count < '{0}'; fi\ncount=$((count+1))\nprintf '%s\\n' \"$count\" > '{0}'\n{page}",
        path.display(),
    );
    let provider = tool(fixture.path(), "sections", script);
    let report = preflight(fixture.path(), &provider, 20);
    assert_eq!(report["preflight"]["complete"], true);
    assert_eq!(report["preflight"]["execution"]["members"], json!([]));
    assert_eq!(std::fs::read_to_string(path).unwrap().trim(), "1");
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

#[test]
fn types() {
    struct Case {
        kind: &'static str,
        heading: &'static str,
        outcome: &'static str,
        missing: bool,
    }
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
