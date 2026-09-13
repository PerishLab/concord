use crate::fixture::{brief, config, run, serve, version};
use std::fs;

#[test]
fn renewed() {
    let root = tempfile::tempdir().unwrap();
    let first = serve(brief());
    let settings = config(root.path(), &first);
    let seat = root.path().join("agent/concord");
    let target = seat.display().to_string();
    let installed = run(&settings, &["skill", "install", "--path", &target]);
    assert!(installed.status.success(), "{:?}", installed);
    let original = fs::read(seat.join("SKILL.md")).unwrap();
    let mut body = brief();
    body.extend_from_slice(b"\nDelivery slices.\n");
    first.update(body.clone());
    let status = run(&settings, &["--json", "skill", "status"]);
    assert!(status.status.success(), "{:?}", status);
    let status: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(status["target"]["version"], version());
    assert_eq!(status["seats"][0]["action"], "upgrade");
    fs::write(seat.join("SKILL.md"), b"local edit").unwrap();
    let refused = run(&settings, &["skill", "upgrade"]);
    assert!(!refused.status.success());
    assert_eq!(fs::read(seat.join("SKILL.md")).unwrap(), b"local edit");
    fs::write(seat.join("SKILL.md"), original).unwrap();
    let upgraded = run(&settings, &["skill", "upgrade"]);
    assert!(upgraded.status.success(), "{:?}", upgraded);
    assert_eq!(fs::read(seat.join("SKILL.md")).unwrap(), body);
    let ledger = fs::read(root.path().join("home/state/skills.json")).unwrap();
    let ledger: serde_json::Value = serde_json::from_slice(&ledger).unwrap();
    assert!(ledger.to_string().contains("/generations/"));
}

#[test]
fn mismatch() {
    let root = tempfile::tempdir().unwrap();
    let source = serve(brief());
    let settings = config(root.path(), &source);
    let seat = root.path().join("agent/concord");
    let result = run(
        &settings,
        &[
            "skill",
            "install",
            "--version",
            "v99.0.0",
            "--path",
            &seat.display().to_string(),
        ],
    );
    assert!(!result.status.success());
    assert!(!seat.exists());
    assert!(!root.path().join("home/state/skills.json").exists());
}

#[test]
fn legacy() {
    let root = tempfile::tempdir().unwrap();
    let source = serve(brief());
    let settings = config(root.path(), &source);
    let seat = root.path().join("agent/concord");
    let installed = run(
        &settings,
        &["skill", "install", "--path", &seat.display().to_string()],
    );
    assert!(installed.status.success(), "{:?}", installed);
    let marker = seat.join("metadata.json");
    let mut held: serde_json::Value = serde_json::from_slice(&fs::read(&marker).unwrap()).unwrap();
    held["version"] = "v0.0.1".into();
    fs::write(&marker, serde_json::to_vec(&held).unwrap()).unwrap();
    let path = root.path().join("home/state/skills.json");
    let mut ledger: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    ledger["records"][0]["version"] = "v0.0.1".into();
    ledger["records"][0]["url"] = "http://127.0.0.1:1/concord-skill.tar.gz".into();
    ledger["records"][0]["sha"] = "b".repeat(64).into();
    fs::write(&path, serde_json::to_vec(&ledger).unwrap()).unwrap();
    let upgraded = run(&settings, &["skill", "upgrade"]);
    assert!(upgraded.status.success(), "{:?}", upgraded);
    let ledger: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(ledger["records"][0]["version"], version());
    assert!(
        ledger["records"][0]["url"]
            .as_str()
            .unwrap()
            .contains("/generations/")
    );
}
