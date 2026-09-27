use serde_json::Value;
use std::path::Path;
use std::process::{Command, Output};

#[test]
fn contract() {
    let fixture = tempfile::tempdir().expect("fixture");
    let root = fixture.path();
    let help = run(root, &["migration", "--help"]);
    assert!(help.status.success());
    let help = String::from_utf8_lossy(&help.stdout);
    for command in ["preflight", "prepare", "apply", "rollback"] {
        assert!(help.contains(command), "missing migration {command}");
    }

    let bootstrap = run(root, &["domain", "bootstrap", "local"]);
    assert!(bootstrap.status.success());
    let refused = run(root, &["--json", "migration", "preflight"]);
    assert!(!refused.status.success());
    let error: Value = serde_json::from_slice(&refused.stderr).expect("error JSON");
    assert_eq!(error["error"]["code"], "concord.migration.source");

    let refused = run(root, &["--json", "migration", "apply", "--plan", "-"]);
    assert!(!refused.status.success());
    let error: Value = serde_json::from_slice(&refused.stderr).expect("error JSON");
    assert_eq!(error["error"]["code"], "concord.apply.required");
}

fn run(root: &Path, arguments: &[&str]) -> Output {
    command(root).args(arguments).output().expect("run Concord")
}

fn command(root: &Path) -> Command {
    let mut command = super::spawn::concord(root);
    command.args(["--root", root.to_str().expect("root path")]);
    command
}
