use super::spawn;
use serde_json::Value;

#[test]
fn query() {
    let fixture = tempfile::tempdir().expect("fixture");
    let code = "concord.estate.upgrade_required";
    let plain = run(fixture.path(), &["cookbook", code]);
    assert!(plain.status.success());
    let plain = String::from_utf8(plain.stdout).expect("human Cookbook output");

    let json = run(fixture.path(), &["--json", "cookbook", code]);
    assert!(json.status.success());
    let json: Value = serde_json::from_slice(&json.stdout).expect("Cookbook JSON");
    let entry = &json["cookbook"]["entries"][0];
    for field in ["code", "trigger", "solution", "evidence", "exit"] {
        let value = entry[field].as_str().expect("Cookbook field");
        assert!(plain.contains(value), "human output omits {field}");
    }
}

#[test]
fn inventory() {
    let fixture = tempfile::tempdir().expect("fixture");
    let listed = run(fixture.path(), &["--json", "cookbook"]);
    assert!(listed.status.success());
    let listed: Value = serde_json::from_slice(&listed.stdout).expect("Cookbook inventory");
    let codes = listed["cookbook"]["entries"]
        .as_array()
        .expect("entries")
        .iter()
        .map(|entry| entry["code"].as_str().expect("code"))
        .collect::<Vec<_>>();
    assert_eq!(
        codes,
        [
            "concord.boundary.refused",
            "concord.delivery.landed",
            "concord.delivery.native",
            "concord.delivery.provider",
            "concord.estate.upgrade_required",
            "concord.issue.needs",
        ]
    );

    let missing = run(fixture.path(), &["--json", "cookbook", "concord.missing"]);
    assert!(!missing.status.success());
    let missing: Value = serde_json::from_slice(&missing.stderr).expect("missing entry error");
    assert_eq!(missing["error"]["code"], "concord.cookbook.absent");
    assert!(missing["error"].get("cookbook").is_none());
}

fn run(scratch: &std::path::Path, arguments: &[&str]) -> std::process::Output {
    spawn::concord(scratch)
        .args(arguments)
        .output()
        .expect("run Concord")
}
