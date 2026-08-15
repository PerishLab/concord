#[path = "seat/spawn.rs"]
mod spawn;

use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;

fn run(home: &Path, arguments: &[&str]) -> Output {
    let mut command = spawn::concord(home);
    command.args(arguments);
    if cfg!(windows) {
        command
            .env("LOCALAPPDATA", home.join("data"))
            .env("USERPROFILE", home.join("profile"));
    } else {
        command.env("HOME", home);
    }
    command.output().expect("run concord")
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
}
