#[path = "../seat/spawn.rs"]
mod spawn;

#[path = "../seat/identity.rs"]
mod identity;

mod observation;

mod refusal;

use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn run(home: &Path, arguments: &[&str]) -> Output {
    process(home, arguments).output().expect("run concord")
}

fn process(scratch: &Path, arguments: &[&str]) -> Command {
    let mut command = spawn::concord(scratch);
    command.args(arguments);
    if cfg!(windows) {
        command
            .env("LOCALAPPDATA", scratch.join("data"))
            .env("USERPROFILE", scratch.join("profile"));
    } else {
        command.env("HOME", scratch);
    }
    command
}

fn expected(home: &Path) -> PathBuf {
    if cfg!(windows) {
        return home.join("data").join("concord").join("concord.toml");
    }
    home.join(".concord").join("concord.toml")
}

fn legacy(home: &Path) -> PathBuf {
    if cfg!(windows) {
        return home
            .join("profile")
            .join("AppData")
            .join("Roaming")
            .join("concord")
            .join("config.toml");
    }
    home.join(".config").join("concord").join("config.toml")
}

#[test]
fn default() {
    let fixture = tempfile::tempdir().expect("fixture");
    let old = legacy(fixture.path());
    fs::create_dir_all(old.parent().unwrap()).unwrap();
    fs::write(old, "space = \"/legacy\"\n").unwrap();
    let output = run(fixture.path(), &["config", "path"]);
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        expected(fixture.path()).display().to_string()
    );
    let output = run(fixture.path(), &["--json", "config", "show"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(value.get("space").is_none());
    assert_eq!(
        value["workspace"],
        expected(fixture.path())
            .parent()
            .unwrap()
            .join("workspace")
            .display()
            .to_string()
    );
    let error = failure(
        &run(fixture.path(), &["--json", "audit"]),
        "concord.estate.absent",
    );
    assert_eq!(
        error["error"]["details"]["provenance"]["kind"],
        "built-in-default"
    );
    assert!(
        !expected(fixture.path())
            .parent()
            .unwrap()
            .join("workspace")
            .exists()
    );
}

#[test]
fn home() {
    let fixture = tempfile::tempdir().unwrap();
    let configured = fixture.path().join("configured");
    let explicit = fixture.path().join("explicit");
    let environment = fixture.path().join("environment");
    let file = fixture.path().join("concord.toml");
    settings(&file, &configured);
    let output = run(
        fixture.path(),
        &["--config", file.to_str().unwrap(), "issue", "bootstrap"],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(configured.join("workspace/identity.json").is_file());
    let output = process(
        fixture.path(),
        &[
            "--config",
            file.to_str().unwrap(),
            "--home",
            explicit.to_str().unwrap(),
            "--json",
            "audit",
        ],
    )
    .env("CONCORD_HOME", &environment)
    .output()
    .unwrap();
    let error = failure(&output, "concord.estate.absent");
    assert_eq!(error["error"]["details"]["home"], path(&explicit));
    assert_eq!(error["error"]["details"]["provenance"]["input"], "--home");
    let output = process(
        fixture.path(),
        &["--config", file.to_str().unwrap(), "--json", "audit"],
    )
    .env("CONCORD_HOME", &environment)
    .output()
    .unwrap();
    let error = failure(&output, "concord.estate.absent");
    assert_eq!(error["error"]["details"]["home"], path(&environment));
    assert_eq!(
        error["error"]["details"]["provenance"]["input"],
        "CONCORD_HOME"
    );
    let output = run(
        fixture.path(),
        &[
            "--config",
            file.to_str().unwrap(),
            "--json",
            "config",
            "show",
        ],
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["provenance"]["kind"], "config");
    assert!(!fixture.path().join(".concord.lock").exists());
    assert!(!fixture.path().join(".issues").exists());
}

fn failure(output: &Output, code: &str) -> Value {
    assert!(!output.status.success());
    let body: Value = serde_json::from_slice(&output.stderr).expect("failure JSON");
    assert_eq!(body["error"]["code"], code);
    body
}

fn directory(root: &Path, name: &str) -> PathBuf {
    let path = root.join(name);
    fs::create_dir(&path).expect("Space root");
    path
}

fn settings(file: &Path, root: &Path) {
    fs::write(file, format!("home = {root:?}\n")).expect("config");
}

fn path(path: &Path) -> String {
    path.canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .display()
        .to_string()
}

#[test]
fn isolated() {
    let fixture = tempfile::tempdir().unwrap();
    let projects = directory(fixture.path(), "Projects");
    let home = fixture.path().join("data");
    let output = process(
        fixture.path(),
        &["--home", home.to_str().unwrap(), "issue", "bootstrap"],
    )
    .current_dir(&projects)
    .output()
    .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::read_dir(&projects).unwrap().count(), 0);
    let before = fs::read(home.join("workspace/identity.json")).unwrap();
    let output = process(
        fixture.path(),
        &["--home", home.to_str().unwrap(), "--json", "audit"],
    )
    .current_dir(&projects)
    .output()
    .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read(home.join("workspace/identity.json")).unwrap(),
        before
    );
    assert_eq!(fs::read_dir(&projects).unwrap().count(), 0);
}
