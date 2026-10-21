use super::super::spawn;
#[cfg(unix)]
use super::planning as fixture;

#[cfg(unix)]
#[test]
fn independent() {
    let root = tempfile::tempdir().unwrap();
    let value = fixture::decoded(&fixture::run(root.path(), "ordinary", "20"));
    assert_eq!(value["schema"], "concord.project-observation/v1");
    assert_eq!(value["complete"], true);
    let fields = &value["collections"]["fields"]["nodes"];
    assert_eq!(fields[0]["name"], fields[1]["name"]);
    assert_eq!(fields[0]["authority"], "project");
    assert_eq!(fields[1]["authority"], "issue");
    let issue = &value["collections"]["items"]["nodes"][0]["content"];
    assert_eq!(issue["state"], "OPEN");
    assert_eq!(
        issue["issueFields"]["nodes"][0]["project_fields"][0],
        "PVTSSF_attached"
    );
    assert_eq!(
        issue["issueFields"]["nodes"][0]["single_select_option"]["name"],
        "High"
    );
    assert!(issue["body"].as_str().unwrap().contains("Acceptance"));
    assert!(!root.path().join(".concord").exists());
    assert!(!root.path().join(".issues").exists());
    let builtin = fixture::decoded(&fixture::run(root.path(), "builtin", "20"));
    assert_eq!(
        builtin["collections"]["fields"]["nodes"][0]["authority"],
        "native-projection"
    );
    let auto = fixture::decoded(&fixture::run(root.path(), "auto", "20"));
    assert_eq!(
        auto["collections"]["items"]["nodes"][0]["custody"],
        "automation-read-only"
    );
}

#[cfg(unix)]
#[test]
fn pagination() {
    let root = tempfile::tempdir().unwrap();
    let value = fixture::decoded(&fixture::run(root.path(), "pages", "20"));
    let fields = &value["collections"]["fields"];
    assert_eq!(fields["nodes"].as_array().unwrap().len(), 2);
    assert_eq!(fields["pages"][1]["requested_after"], "next");
    assert_eq!(value["complete"], true);
    let value = fixture::decoded(&fixture::run(root.path(), "pages", "1"));
    assert_eq!(value["complete"], false);
    assert_eq!(value["diagnostics"][0]["code"], "page-budget");
    assert_eq!(
        value["collections"]["fields"]["pages"][0]["end_cursor"],
        "next"
    );
}

#[cfg(unix)]
#[test]
fn incomplete() {
    let root = tempfile::tempdir().unwrap();
    for (mode, code) in [
        ("null", "unknown-content-or-type"),
        ("nested", "nested-budget"),
    ] {
        let value = fixture::decoded(&fixture::run(root.path(), mode, "20"));
        assert_eq!(value["complete"], false);
        assert_eq!(value["diagnostics"][0]["code"], code);
    }
}

#[cfg(unix)]
#[test]
fn refusals() {
    let root = tempfile::tempdir().unwrap();
    for (mode, code) in [
        ("permission", "provider"),
        ("missing", "unavailable"),
        ("identity", "identity"),
        ("changed", "changed"),
        ("schema", "schema"),
        ("origin", "schema"),
        ("missing-connection", "schema"),
        ("repeat", "changed"),
        ("cursor", "schema"),
    ] {
        let output = fixture::run(root.path(), mode, "20");
        assert!(!output.status.success(), "{mode}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(&format!("concord.project.{code}")),
            "{mode}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn grammar() {
    let scratch = tempfile::tempdir().unwrap();
    for target in [
        "PerishLab/0",
        "https://other.test/orgs/PerishLab/projects/1",
        "PerishLab/1/2",
    ] {
        let output = spawn::concord(scratch.path())
            .args([
                "project",
                "show",
                target,
                "--github-command",
                "missing-provider",
                "--json",
            ])
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("concord.project.target"));
    }
    let output = spawn::concord(scratch.path())
        .args(["project", "show", "--help"])
        .output()
        .unwrap();
    let help = String::from_utf8_lossy(&output.stdout);
    assert!(help.contains("without opening local execution state"));
    assert!(help.contains("never an atomic snapshot"));
    assert!(!scratch.path().join(".concord").exists());
}
