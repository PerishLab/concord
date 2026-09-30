#[path = "seat/spawn.rs"]
mod spawn;

#[path = "seat/identity.rs"]
mod identity;

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
    fs::create_dir_all(old.parent().expect("legacy parent")).expect("legacy parent");
    fs::write(&old, "domain_space_root = \"/legacy\"\n").expect("legacy config");

    let path = run(fixture.path(), &["config", "path"]);
    assert!(
        path.status.success(),
        "{}",
        String::from_utf8_lossy(&path.stderr)
    );
    assert_eq!(
        String::from_utf8(path.stdout).expect("config path").trim(),
        expected(fixture.path()).display().to_string()
    );

    let shown = run(fixture.path(), &["--json", "config", "show"]);
    assert!(
        shown.status.success(),
        "{}",
        String::from_utf8_lossy(&shown.stderr)
    );
    let config: Value = serde_json::from_slice(&shown.stdout).expect("config json");
    assert_eq!(config["domain_space_root"], "");
    assert_eq!(config["depot"], "https://depot.concord.perish.uk");

    let missing = run(fixture.path(), &["--json", "audit"]);
    let missing = failure(&missing, "concord.space.root");
    assert_eq!(missing["error"]["details"]["root"], "");
    assert_eq!(
        missing["error"]["details"]["provenance"]["kind"],
        "built-in-default"
    );
}

#[test]
fn space() {
    let fixture = tempfile::tempdir().expect("fixture");
    let configured = directory(fixture.path(), "configured");
    let explicit = directory(fixture.path(), "explicit");
    let environment = directory(fixture.path(), "environment");
    let file = fixture.path().join("concord.toml");
    settings(&file, &configured);
    let bootstrapped = run(
        fixture.path(),
        &[
            "--config",
            file.to_str().expect("config"),
            "issue",
            "bootstrap",
        ],
    );
    assert!(bootstrapped.status.success());

    let absent = run(
        fixture.path(),
        &[
            "--config",
            file.to_str().expect("config"),
            "--root",
            explicit.to_str().expect("explicit root"),
            "--json",
            "audit",
        ],
    );
    let absent = failure(&absent, "concord.estate.absent");
    assert_eq!(absent["error"]["details"]["root"], path(&explicit));
    assert_eq!(
        absent["error"]["details"]["estate"],
        path(&explicit.join(".concord"))
    );
    assert_eq!(
        absent["error"]["details"]["provenance"],
        serde_json::json!({"kind": "explicit", "input": "--root"})
    );
    assert_eq!(
        absent["error"]["details"]["configured_alternative"]["root"],
        path(&configured)
    );
    assert_eq!(
        absent["error"]["details"]["configured_alternative"]["provenance"]["path"],
        path(&file)
    );

    let human = run(
        fixture.path(),
        &[
            "--config",
            file.to_str().expect("config"),
            "--root",
            explicit.to_str().expect("explicit root"),
            "audit",
        ],
    );
    let prose = String::from_utf8_lossy(&human.stderr);
    assert!(prose.contains("remove or correct --root"), "{prose}");
    assert!(prose.contains("concord config show"), "{prose}");

    let missing = directory(fixture.path(), "configured-absent");
    settings(&file, &missing);
    let report = run(
        fixture.path(),
        &[
            "--config",
            file.to_str().expect("config"),
            "--json",
            "audit",
        ],
    );
    let report = failure(&report, "concord.estate.absent");
    assert_eq!(report["error"]["details"]["provenance"]["kind"], "config");
    assert_eq!(
        report["error"]["details"]["provenance"]["input"],
        "--config"
    );

    let environmental = process(fixture.path(), &["--json", "audit"])
        .env("CONCORD_DOMAIN_SPACE_ROOT", &environment)
        .output()
        .expect("environment root");
    let environmental = failure(&environmental, "concord.estate.absent");
    assert_eq!(
        environmental["error"]["details"]["provenance"]["input"],
        "CONCORD_DOMAIN_SPACE_ROOT"
    );

    let conflict = process(
        fixture.path(),
        &[
            "--config",
            file.to_str().expect("config"),
            "--root",
            explicit.to_str().expect("explicit root"),
            "--json",
            "audit",
        ],
    )
    .env("CONCORD_DOMAIN_SPACE_ROOT", &environment)
    .output()
    .expect("conflicting roots");
    let conflict = failure(&conflict, "concord.estate.absent");
    assert_eq!(
        conflict["error"]["details"]["configured_alternative"]["provenance"]["kind"],
        "environment"
    );

    let default = expected(fixture.path());
    fs::create_dir_all(default.parent().expect("default config parent"))
        .expect("default config parent");
    settings(&default, &missing);
    let report = run(fixture.path(), &["--json", "audit"]);
    let report = failure(&report, "concord.estate.absent");
    assert_eq!(
        report["error"]["details"]["provenance"]["input"],
        "platform-default"
    );
    assert_eq!(
        report["error"]["details"]["provenance"]["path"],
        path(&default)
    );
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
    fs::write(file, format!("domain_space_root = {root:?}\n")).expect("config");
}

fn path(path: &Path) -> String {
    path.canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .display()
        .to_string()
}
