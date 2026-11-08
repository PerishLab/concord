use super::provider;

use serde_json::Value;

fn decoded(root: &std::path::Path, mode: &str, extra: &[&str]) -> Value {
    let output = provider::run(root, mode, extra);
    assert!(
        output.status.success(),
        "{mode}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice::<Value>(&output.stdout).unwrap()["survey"].clone()
}

#[test]
fn shared() {
    let root = tempfile::tempdir().unwrap();
    let value = decoded(root.path(), "ordinary", &[]);
    assert_eq!(value["complete"], true);
    assert_eq!(
        value["coverage"]["counts"],
        serde_json::json!({"ordinary":2,"auto":1,"unknown":1})
    );
    let issues = value["coverage"]["issues"].as_array().unwrap();
    assert_eq!(issues.len(), 4);
    assert_eq!(issues[0]["memberships"].as_array().unwrap().len(), 2);
    assert_eq!(
        issues[0]["memberships"][0]["values"]["nodes"][0]["text"],
        "Different benefit 1"
    );
    assert_eq!(
        issues[0]["memberships"][1]["values"]["nodes"][0]["text"],
        "Different benefit 2"
    );
    assert_eq!(value["coverage"]["projects"][0]["missing"][0]["id"], "I2");
    assert_eq!(
        value["coverage"]["projects"][0]["missing"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let calls = std::fs::read_to_string(root.path().join("calls")).unwrap();
    assert!(!calls.contains("mutation"));
    assert!(!calls.contains("comments("));
    assert!(!root.path().join("workspace").exists());
    assert!(!root.path().join("workspace/issues").exists());
}

#[test]
fn planning() {
    let root = tempfile::tempdir().unwrap();
    for (mode, expected) in [
        ("ordinary", "no-current-cycle"),
        ("ready", "ready"),
        ("missing-fields", "missing-fields"),
        ("ambiguous-fields", "ambiguous-fields"),
        ("ambiguous-cycle", "ambiguous-cycle"),
    ] {
        let value = decoded(root.path(), mode, &[]);
        assert_eq!(
            value["projects"][0]["planning"]["status"], expected,
            "{mode}"
        );
    }
}

#[test]
fn pages() {
    let root = tempfile::tempdir().unwrap();
    let value = decoded(root.path(), "pages", &[]);
    assert_eq!(value["complete"], true);
    assert_eq!(
        value["repositories"]["inventory"]["pages"][1]["requested_after"],
        "next"
    );
    assert_eq!(
        value["repositories"]["observations"][0]["issues"]["pages"][1]["requested_after"],
        "next"
    );
    let value = decoded(root.path(), "pages", &["--max-pages", "1"]);
    assert_eq!(value["complete"], false);
    assert!(value["coverage"]["projects"][0]["missing"].is_null());
    for mode in ["field-page", "item-page", "nested", "unknown-content"] {
        let value = decoded(root.path(), mode, &["--max-pages", "1"]);
        assert_eq!(value["complete"], false, "{mode}");
        assert!(
            value["coverage"]["projects"][0]["missing"].is_null(),
            "{mode}"
        );
    }
}

#[test]
fn refuses() {
    let root = tempfile::tempdir().unwrap();
    for mode in [
        "permission",
        "foreign",
        "issue-foreign",
        "project-foreign",
        "item-foreign",
        "repeat",
        "changed",
        "changed-issue",
    ] {
        let output = provider::run(root.path(), mode, &[]);
        assert!(!output.status.success(), "{mode}");
        assert!(output.stdout.is_empty(), "{mode}");
    }
    let output = provider::run(root.path(), "pages", &["--max-repositories", "1"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("bounds"));
    let output = provider::run(root.path(), "ordinary", &["--max-issues", "1"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("bounds"));
}
