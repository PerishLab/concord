#[path = "skill/fixture.rs"]
mod fixture;

use fixture::{archive, config, run, serve};
use std::fs;

#[test]
fn product_skill_install_refusal_and_uninstall_close_the_loop() {
    let fixture = tempfile::tempdir().expect("fixture");
    let releases = serve(archive());
    let config_path = config(fixture.path(), &releases);
    let argument_home = fixture.path().join("argument-home");
    let argument_home_text = argument_home.display().to_string();
    let skill = fixture.path().join("agent/skills/concord");
    let skill_text = skill.display().to_string();

    let installed = run(
        &config_path,
        &[
            "--json",
            "--home",
            &argument_home_text,
            "skill",
            "install",
            "--version",
            "v0.3.0",
            "--path",
            &skill_text,
        ],
    );
    assert!(
        installed.status.success(),
        "{}",
        String::from_utf8_lossy(&installed.stderr)
    );
    assert!(skill.join("SKILL.md").is_file());
    assert!(skill.join("references/protocol.md").is_file());
    let marker = fs::read_to_string(skill.join("metadata.json")).expect("marker");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&marker).expect("marker json")["keeper"],
        "concord"
    );
    assert!(argument_home.join("state/skills.json").is_file(), "ledger");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mode = fs::metadata(argument_home.join("state/skills.json"))
            .expect("ledger")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600, "ledger stays private");
    }
    assert!(
        !fixture.path().join("home/state/skills.json").exists(),
        "argument layer must override file home"
    );

    let listed = run(
        &config_path,
        &["--home", &argument_home_text, "skill", "list"],
    );
    assert!(listed.status.success());
    assert!(String::from_utf8_lossy(&listed.stdout).contains(&skill_text));

    let upgraded_releases = serve(archive());
    let upgraded_config = config(fixture.path(), &upgraded_releases);
    let status = run(
        &upgraded_config,
        &[
            "--json",
            "--home",
            &argument_home_text,
            "skill",
            "status",
            "--version",
            "v0.3.0",
        ],
    );
    assert!(status.status.success());
    let status: serde_json::Value = serde_json::from_slice(&status.stdout).expect("status json");
    assert_eq!(status["operation"], "status");
    assert_eq!(status["seats"][0]["state"], "current");
    assert_eq!(status["seats"][0]["action"], "none");

    let dry_run = run(
        &upgraded_config,
        &[
            "--json",
            "--home",
            &argument_home_text,
            "skill",
            "upgrade",
            "--version",
            "v0.3.0",
            "--dry-run",
        ],
    );
    assert!(dry_run.status.success());
    let dry_run: serde_json::Value = serde_json::from_slice(&dry_run.stdout).expect("dry-run json");
    assert_eq!(dry_run["operation"], "upgrade_dry_run");
    assert_eq!(dry_run["seats"][0]["action"], "none");

    let upgraded = run(
        &upgraded_config,
        &[
            "--json",
            "--home",
            &argument_home_text,
            "skill",
            "upgrade",
            "--version",
            "v0.3.0",
        ],
    );
    assert!(
        upgraded.status.success(),
        "{}",
        String::from_utf8_lossy(&upgraded.stderr)
    );
    let upgraded: serde_json::Value =
        serde_json::from_slice(&upgraded.stdout).expect("upgrade json");
    assert!(upgraded["changed"].as_array().expect("changed").is_empty());
    assert_eq!(
        upgraded["unchanged"].as_array().expect("unchanged").len(),
        1
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mode = fs::metadata(argument_home.join("state/skills.json"))
            .expect("ledger")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600, "ledger replacement stays private");
    }

    fs::write(skill.join("metadata.json"), "{}").expect("spoil marker");
    let refused = run(
        &upgraded_config,
        &["--home", &argument_home_text, "skill", "uninstall"],
    );
    assert!(!refused.status.success(), "one-sided ownership must refuse");
    assert!(skill.is_dir(), "refused target survives");

    fs::write(skill.join("metadata.json"), marker).expect("restore marker");
    let removed = run(
        &upgraded_config,
        &["--home", &argument_home_text, "skill", "uninstall"],
    );
    assert!(
        removed.status.success(),
        "{}",
        String::from_utf8_lossy(&removed.stderr)
    );
    assert!(!skill.exists(), "managed target removed exactly");
}

#[test]
fn unmanaged_status_is_diagnostic_but_dry_run_refuses() {
    let fixture = tempfile::tempdir().expect("fixture");
    let releases = serve(archive());
    let config_path = config(fixture.path(), &releases);
    let home = fixture.path().join("argument-home");
    let home = home.display().to_string();

    let status = run(
        &config_path,
        &["--json", "--home", &home, "skill", "status"],
    );
    assert!(status.status.success());
    let status: serde_json::Value = serde_json::from_slice(&status.stdout).expect("status json");
    assert!(status["seats"].as_array().expect("seats").is_empty());

    let dry_run = run(
        &config_path,
        &["--home", &home, "skill", "upgrade", "--dry-run"],
    );
    assert!(!dry_run.status.success());
    assert!(String::from_utf8_lossy(&dry_run.stderr).contains("not actionable"));
}

#[test]
fn product_skill_stage_isolated() {
    let fixture = tempfile::tempdir().expect("fixture");
    let releases = serve(archive());
    let config_path = config(fixture.path(), &releases);
    let home = fixture.path().join("argument-home");
    let staged = fixture.path().join("candidate/skills/concord");

    let output = run(
        &config_path,
        &[
            "--home",
            &home.display().to_string(),
            "skill",
            "stage",
            "--channel",
            "beta",
            "--version",
            "v0.5.0-beta.1",
            "--path",
            &staged.display().to_string(),
        ],
    );

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(staged.join("SKILL.md").is_file());
    assert!(!home.join("state/skills.json").exists());
}
