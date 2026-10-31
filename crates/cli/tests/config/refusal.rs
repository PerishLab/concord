use super::{directory, expected, failure, path, process, run, settings};
use std::fs;

#[test]
fn environment() {
    let fixture = tempfile::tempdir().expect("fixture");
    let space = directory(fixture.path(), "space");
    for legacy in ["", "missing", space.to_str().expect("space")] {
        let output = process(
            fixture.path(),
            &[
                "--space",
                space.to_str().expect("space"),
                "--json",
                "config",
                "show",
            ],
        )
        .env("CONCORD_SPACE", &space)
        .env("CONCORD_DOMAIN_SPACE_ROOT", legacy)
        .output()
        .expect("legacy environment");
        let error = failure(&output, "concord.config.legacy");
        assert!(
            error["error"]["message"]
                .as_str()
                .expect("message")
                .contains("CONCORD_SPACE")
        );
    }
}

#[test]
fn file() {
    let fixture = tempfile::tempdir().expect("fixture");
    let space = directory(fixture.path(), "space");
    let file = fixture.path().join("concord.toml");
    for value in ["\"\"", "\"/legacy\"", "42", "false", "{ path = \"old\" }"] {
        fs::write(
            &file,
            format!("space = {space:?}\ndomain_space_root = {value}\n"),
        )
        .expect("legacy config");
        let output = process(
            fixture.path(),
            &[
                "--config",
                file.to_str().expect("file"),
                "--json",
                "config",
                "show",
            ],
        )
        .env("CONCORD_SPACE", &space)
        .output()
        .expect("legacy config");
        let error = failure(&output, "concord.config.legacy");
        let message = error["error"]["message"].as_str().expect("message");
        assert!(message.contains("rename that key to space"), "{message}");
        assert!(message.contains(file.to_str().expect("file")), "{message}");
    }
    let file = expected(fixture.path());
    fs::create_dir_all(file.parent().expect("parent")).expect("parent");
    fs::write(file, "domain_space_root = \"/legacy\"\n").expect("default legacy");
    failure(
        &run(fixture.path(), &["--json", "config", "show"]),
        "concord.config.legacy",
    );
    assert!(run(fixture.path(), &["config", "path"]).status.success());
}

#[test]
fn grammar() {
    let fixture = tempfile::tempdir().expect("fixture");
    let help = run(fixture.path(), &["--help"]);
    assert!(help.status.success());
    let help = String::from_utf8(help.stdout).expect("help");
    assert!(help.contains("--space <SPACE>"), "{help}");
    assert!(!help.contains("--root"), "{help}");
    assert!(
        !run(fixture.path(), &["--root", "/legacy", "config", "show"])
            .status
            .success()
    );
}

#[test]
fn precedence() {
    let fixture = tempfile::tempdir().expect("fixture");
    let configured = directory(fixture.path(), "configured");
    let environment = directory(fixture.path(), "environment");
    let explicit = directory(fixture.path(), "explicit");
    let file = fixture.path().join("concord.toml");
    settings(&file, &configured);
    for (arguments, selected) in [
        (
            vec![
                "--config",
                file.to_str().expect("file"),
                "--json",
                "config",
                "show",
            ],
            &environment,
        ),
        (
            vec![
                "--config",
                file.to_str().expect("file"),
                "--space",
                explicit.to_str().expect("explicit"),
                "--json",
                "config",
                "show",
            ],
            &explicit,
        ),
    ] {
        let output = process(fixture.path(), &arguments)
            .env("CONCORD_SPACE", &environment)
            .output()
            .expect("config");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let shown: serde_json::Value = serde_json::from_slice(&output.stdout).expect("config JSON");
        assert_eq!(shown["space"], path(selected));
        assert!(shown.get("domain_space_root").is_none());
    }
}
