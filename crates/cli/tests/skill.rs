#[path = "skill/fixture.rs"]
mod fixture;
#[path = "seat/spawn.rs"]
mod spawn;

use fixture::{archive, config, run, serve};
use std::fs;

#[test]
fn lifecycle() {
    let fixture = tempfile::tempdir().expect("fixture");
    let releases = serve(archive());
    let settings = config(fixture.path(), &releases);
    let home = fixture.path().join("argument-home");
    let text = home.display().to_string();
    let skill = fixture.path().join("agent/skills/concord");
    let target = skill.display().to_string();

    let installed = run(
        &settings,
        &[
            "--json",
            "--home",
            &text,
            "skill",
            "install",
            "--version",
            "v0.3.0",
            "--path",
            &target,
        ],
    );
    assert!(
        installed.status.success(),
        "{}",
        String::from_utf8_lossy(&installed.stderr)
    );
    assert!(skill.join("SKILL.md").is_file());
    assert!(!skill.join("references").exists());
    let marker = fs::read_to_string(skill.join("metadata.json")).expect("marker");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&marker).expect("marker json")["keeper"],
        "concord"
    );
    assert!(home.join("state/skills.json").is_file(), "ledger");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mode = fs::metadata(home.join("state/skills.json"))
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

    let listed = run(&settings, &["--home", &text, "skill", "list"]);
    assert!(listed.status.success());
    assert!(String::from_utf8_lossy(&listed.stdout).contains(&target));

    let release = serve(archive());
    let next = config(fixture.path(), &release);
    let status = run(
        &next,
        &[
            "--json",
            "--home",
            &text,
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

    let preview = run(
        &next,
        &[
            "--json",
            "--home",
            &text,
            "skill",
            "upgrade",
            "--version",
            "v0.3.0",
            "--dry-run",
        ],
    );
    assert!(preview.status.success());
    let preview: serde_json::Value = serde_json::from_slice(&preview.stdout).expect("dry-run json");
    assert_eq!(preview["operation"], "upgrade_dry_run");
    assert_eq!(preview["seats"][0]["action"], "none");

    let upgraded = run(
        &next,
        &[
            "--json",
            "--home",
            &text,
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

        let mode = fs::metadata(home.join("state/skills.json"))
            .expect("ledger")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600, "ledger replacement stays private");
    }

    fs::write(skill.join("metadata.json"), "{}").expect("spoil marker");
    let refused = run(&next, &["--home", &text, "skill", "uninstall"]);
    assert!(!refused.status.success(), "one-sided ownership must refuse");
    assert!(skill.is_dir(), "refused target survives");

    fs::write(skill.join("metadata.json"), marker).expect("restore marker");
    let removed = run(&next, &["--home", &text, "skill", "uninstall"]);
    assert!(
        removed.status.success(),
        "{}",
        String::from_utf8_lossy(&removed.stderr)
    );
    assert!(!skill.exists(), "managed target removed exactly");
}

#[test]
fn unmanaged() {
    let fixture = tempfile::tempdir().expect("fixture");
    let releases = serve(archive());
    let settings = config(fixture.path(), &releases);
    let home = fixture.path().join("argument-home");
    let home = home.display().to_string();

    let status = run(&settings, &["--json", "--home", &home, "skill", "status"]);
    assert!(status.status.success());
    let status: serde_json::Value = serde_json::from_slice(&status.stdout).expect("status json");
    assert!(status["seats"].as_array().expect("seats").is_empty());

    let preview = run(
        &settings,
        &["--home", &home, "skill", "upgrade", "--dry-run"],
    );
    assert!(!preview.status.success());
    assert!(String::from_utf8_lossy(&preview.stderr).contains("not actionable"));
}

#[test]
fn staging() {
    let fixture = tempfile::tempdir().expect("fixture");
    let releases = serve(archive());
    let settings = config(fixture.path(), &releases);
    let home = fixture.path().join("argument-home");
    let staged = fixture.path().join("candidate/skills/concord");

    let output = run(
        &settings,
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
    assert!(!staged.join("references").exists());
    assert!(!home.join("state/skills.json").exists());
}
