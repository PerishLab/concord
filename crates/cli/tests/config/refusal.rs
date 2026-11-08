use super::{directory, expected, failure, process, run};
use std::fs;

#[test]
fn environment() {
    let fixture = tempfile::tempdir().unwrap();
    let home = directory(fixture.path(), "data");
    for variable in ["CONCORD_SPACE", "CONCORD_DOMAIN_SPACE_ROOT"] {
        for value in ["", "missing", home.to_str().unwrap()] {
            let output = process(
                fixture.path(),
                &["--home", home.to_str().unwrap(), "--json", "config", "show"],
            )
            .env(variable, value)
            .output()
            .unwrap();
            let error = failure(&output, "concord.config.legacy");
            assert!(
                error["error"]["message"]
                    .as_str()
                    .unwrap()
                    .contains("data home")
            );
        }
    }
}

#[test]
fn file() {
    let fixture = tempfile::tempdir().unwrap();
    let file = fixture.path().join("concord.toml");
    for key in ["space", "domain_space_root"] {
        for value in ["\"\"", "\"/legacy\"", "42", "false", "{ path = \"old\" }"] {
            fs::write(&file, format!("{key} = {value}\n")).unwrap();
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
            failure(&output, "concord.config.legacy");
        }
    }
    let file = expected(fixture.path());
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(&file, "space = \"/legacy\"\n").unwrap();
    assert!(run(fixture.path(), &["config", "path"]).status.success());
}

#[test]
fn grammar() {
    let fixture = tempfile::tempdir().unwrap();
    let output = run(fixture.path(), &["--help"]);
    let help = String::from_utf8(output.stdout).unwrap();
    assert!(!help.contains("--space"));
    assert!(!help.contains("--root"));
    let output = run(
        fixture.path(),
        &[
            "--space",
            fixture.path().to_str().unwrap(),
            "--json",
            "audit",
        ],
    );
    failure(&output, "concord.config.legacy");
    assert!(
        !run(fixture.path(), &["--root", ".", "audit"])
            .status
            .success()
    );
}
