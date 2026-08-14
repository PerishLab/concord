use serde_json::Value;
use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn envelope() {
    let change = help(&["task", "change", "--help"]);
    assert!(change.contains("\"edits\""), "{change}");
    assert!(change.contains("\"op\": \"set\""), "{change}");
    assert!(change.contains("\"revision\""), "{change}");

    let settle = help(&["phase", "settle", "--help"]);
    assert!(settle.contains("\"phase\""), "{settle}");
    assert!(settle.contains("\"part\": \"outcome\""), "{settle}");

    let fixture = tempfile::tempdir().expect("fixture");
    let root = fixture.path().to_str().expect("root path");
    let bootstrap = Command::new(env!("CARGO_BIN_EXE_concord"))
        .args(["--root", root, "domain", "bootstrap", "local"])
        .output()
        .expect("bootstrap estate");
    assert!(bootstrap.status.success());

    let refused = refuse(root, &["--json", "task", "change"], "{\"version\":1}");
    assert_eq!(refused["error"]["code"], "concord.input.json");
    let envelope = &refused["error"]["details"]["envelope"];
    assert_eq!(envelope["version"], 1);
    assert!(envelope["task"].is_string());
    assert!(envelope["edits"].is_array());

    let settled = refuse(root, &["--json", "phase", "settle"], "{\"version\":1}");
    assert_eq!(settled["error"]["code"], "concord.input.json");
    assert!(settled["error"]["details"]["envelope"]["phase"].is_array());
}

fn help(arguments: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_concord"))
        .args(arguments)
        .output()
        .expect("run Concord");
    assert!(output.status.success());
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn refuse(root: &str, arguments: &[&str], body: &str) -> Value {
    let mut child = Command::new(env!("CARGO_BIN_EXE_concord"))
        .args(["--root", root])
        .args(arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn Concord");
    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(body.as_bytes())
        .expect("write envelope");
    let output = child.wait_with_output().expect("run Concord");
    assert!(!output.status.success());
    serde_json::from_slice(&output.stderr).expect("error JSON")
}
