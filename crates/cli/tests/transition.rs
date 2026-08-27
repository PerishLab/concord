use std::path::PathBuf;
use std::process::Command;

const UNIX: &str = include_str!("fixtures/transition/migration.sh");
const WINDOWS: &str = include_str!("fixtures/transition/migration.ps1");

fn artifact(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/transition")
        .join(name)
}

#[test]
fn pair() {
    for script in [UNIX, WINDOWS] {
        assert!(script.contains("v0.11.0"));
        assert!(script.contains("v0.10.0"));
        assert!(script.contains("manager digest mismatch"));
        assert!(script.contains("temporary"));
        assert!(script.contains("--install-root"));
        assert!(script.contains("--bin-dir"));
    }
    assert!(UNIX.contains(
        "https://releases.concord.perish.uk/v1/objects/sha256/0e893486a22b5b189a58367323588755a965aa2e1f4737f2d2a4aac8906a9e77/manage.sh"
    ));
    assert!(WINDOWS.contains(
        "https://releases.concord.perish.uk/v1/objects/sha256/bd8eb9515100feeb22550b3176b09831b1c7f4881059d78bacbd5ceabd5c05b6/manage.ps1"
    ));
}

#[cfg(unix)]
#[test]
fn shell() {
    let path = artifact("migration.sh");
    let syntax = Command::new("sh")
        .arg("-n")
        .arg(&path)
        .status()
        .expect("shell syntax should run");
    assert!(syntax.success());
    let help = Command::new("sh")
        .arg(&path)
        .arg("--help")
        .output()
        .expect("shell help should run");
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("--root SPACE"));
}

#[cfg(unix)]
fn unix(root: &std::path::Path, arguments: &[&str]) -> std::process::Output {
    Command::new("sh")
        .arg(artifact("migration.sh"))
        .args([
            "--root",
            root.to_str().expect("fixture root should be UTF-8"),
        ])
        .args(arguments)
        .output()
        .expect("shell migration refusal should run")
}

#[cfg(unix)]
#[test]
fn refusal() {
    let unknown = tempfile::tempdir().expect("unknown fixture");
    std::fs::create_dir(unknown.path().join(".concord")).expect("unknown estate");
    std::fs::write(unknown.path().join(".concord/foreign"), "occupied\n").expect("unknown entry");
    let output = unix(unknown.path(), &["survey"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unknown migration territory"));

    let active = tempfile::tempdir().expect("active fixture");
    std::fs::create_dir(active.path().join(".concord")).expect("active estate");
    std::fs::write(active.path().join(".concord/estate.sqlite3"), "occupied\n")
        .expect("active database");
    let output = unix(active.path(), &["survey"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("active or partial estate"));

    let target = tempfile::tempdir().expect("link target");
    let seat = tempfile::tempdir().expect("link seat");
    let link = seat.path().join("space");
    std::os::unix::fs::symlink(target.path(), &link).expect("Space link");
    let output = unix(&link, &["survey"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("one real directory"));

    let explicit = tempfile::tempdir().expect("activation fixture");
    let output = unix(explicit.path(), &["activate", "fingerprint"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("FINGERPRINT --apply"));
}

#[cfg(windows)]
#[test]
fn powershell() {
    let help = Command::new("powershell")
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
        ])
        .arg(artifact("migration.ps1"))
        .arg("-Help")
        .output()
        .expect("PowerShell help should run");
    assert!(
        help.status.success(),
        "PowerShell help failed: {}",
        String::from_utf8_lossy(&help.stderr)
    );
    assert!(String::from_utf8_lossy(&help.stdout).contains("-Root SPACE"));
}
