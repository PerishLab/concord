use concord_core::{Memory, Root, Space};
use serde_json::Value;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

fn run(root: &Path, arguments: &[&str], stdin: &[u8]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_concord"))
        .arg("--root")
        .arg(root)
        .args(arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run concord");
    child
        .stdin
        .take()
        .expect("child stdin")
        .write_all(stdin)
        .expect("write child stdin");
    child.wait_with_output().expect("wait for concord")
}

fn main(focus: &str, next: &str) -> String {
    format!(
        "<!-- concord-memory:v1 -->\n# Brief\n\n## Goal\n\nShip.\n\n## Active constraints\n\nKeep bytes.\n\n## Decisions in force\n\nUse CAS.\n\n## Current focus\n\n{focus}\n\n## Open questions\n\nNone.\n\n## Next step\n\n{next}\n"
    )
}

fn phase() -> &'static str {
    "<!-- concord-phase:v1 -->\n# Slice\n\n## Outcome\n\nDone.\n\n## Decisions\n\nKeep CAS.\n\n## Evidence\n\nTests.\n\n## Carry-forward\n\nContinue.\n"
}

#[test]
fn projection_patch_and_noop_form_one_sparse_cas_loop() {
    let fixture = tempfile::tempdir().expect("fixture");
    let root = Root::new(fixture.path()).expect("canonical root");
    let space = Space::new(root);
    space.domain_init("local", true).expect("initialize domain");
    space.task_start("local/v1", true).expect("start task");
    let task = space.resolve("local/v1").expect("resolve task");
    let memory = Memory::new(&task);
    memory.init(&main("Parser.", "Test.")).expect("init v1");

    let projection = run(
        fixture.path(),
        &[
            "--json",
            "memory",
            "read",
            "local/v1",
            "--section",
            "focus",
            "--section",
            "next",
        ],
        b"",
    );
    assert!(projection.status.success());
    let projection: Value = serde_json::from_slice(&projection.stdout).expect("projection JSON");
    let revision = projection["revision"].as_str().expect("revision");
    let content = projection["content"].as_str().expect("patch content");
    assert!(content.starts_with("<!-- concord-memory-patch:v1 revision="));
    assert!(!content.contains("## Goal"));
    assert!(content.contains("## Current focus"));
    assert!(content.contains("## Next step"));
    let human = run(
        fixture.path(),
        &[
            "memory",
            "read",
            "local/v1",
            "--section",
            "focus",
            "--section",
            "next",
        ],
        b"",
    );
    assert_eq!(
        String::from_utf8(human.stdout).expect("human patch"),
        content
    );

    let edited = content.replace("Parser.", "Raw splice.");
    let patched = run(
        fixture.path(),
        &["--json", "memory", "patch", "local/v1", "--file", "-"],
        edited.as_bytes(),
    );
    assert!(
        patched.status.success(),
        "{}",
        String::from_utf8_lossy(&patched.stderr)
    );
    let patched: Value = serde_json::from_slice(&patched.stdout).expect("patch JSON");
    assert_eq!(patched["result"]["changed"], true);
    assert_eq!(
        memory.read().expect("updated memory").content,
        main("Raw splice.", "Test.")
    );

    let projected = memory
        .read_sections(&["focus".to_string()])
        .expect("fresh projection");
    let noop = run(
        fixture.path(),
        &["--json", "memory", "patch", "local/v1", "--file", "-"],
        projected.content.as_bytes(),
    );
    assert!(noop.status.success());
    let noop: Value = serde_json::from_slice(&noop.stdout).expect("noop JSON");
    assert_eq!(noop["result"]["changed"], false);
    assert_eq!(noop["result"]["revision"], projected.revision);

    let mismatch = run(
        fixture.path(),
        &[
            "--json", "memory", "patch", "local/v1", "--expect", revision, "--file", "-",
        ],
        projected.content.as_bytes(),
    );
    assert!(!mismatch.status.success());
    let error: Value = serde_json::from_slice(&mismatch.stderr).expect("error JSON");
    assert_eq!(error["error"]["code"], "memory.patch_expect");

    let current = memory.read().expect("current structured memory");
    let downgrade = memory
        .write(&current.revision, "# Legacy\n")
        .expect_err("ordinary write cannot downgrade");
    assert_eq!(downgrade.code(), "memory.downgrade");
    assert_eq!(
        memory.read().expect("still structured").revision,
        current.revision
    );
}

#[test]
fn structured_settle_lists_phases_and_refuses_an_unchanged_main() {
    let fixture = tempfile::tempdir().expect("fixture");
    let root = Root::new(fixture.path()).expect("canonical root");
    let space = Space::new(root);
    space.domain_init("local", true).expect("initialize domain");
    space.task_start("local/v1", true).expect("start task");
    let task = space.resolve("local/v1").expect("resolve task");
    let memory = Memory::new(&task);
    memory.init(&main("Parser.", "Settle.")).expect("init v1");
    let held = memory.read().expect("held main");
    let next = main("Audit.", "Continue.");
    let settled = run(
        fixture.path(),
        &[
            "--json",
            "memory",
            "settle",
            "local/v1",
            "--expect",
            &held.revision,
            "--phase-file",
            "-",
            "--main-file",
            "NEXT.md",
        ],
        phase().as_bytes(),
    );
    assert!(!settled.status.success(), "missing file must quick fail");
    std::fs::write(fixture.path().join("NEXT.md"), &next).expect("write next");
    let settled = run(
        fixture.path(),
        &[
            "--json",
            "memory",
            "settle",
            "local/v1",
            "--expect",
            &held.revision,
            "--phase-file",
            "-",
            "--main-file",
            fixture.path().join("NEXT.md").to_str().expect("next path"),
        ],
        phase().as_bytes(),
    );
    assert!(settled.status.success());

    let listed = run(
        fixture.path(),
        &["--json", "memory", "phase", "list", "local/v1"],
        b"",
    );
    let listed: Value = serde_json::from_slice(&listed.stdout).expect("phase list");
    assert_eq!(listed[0]["number"], 0);
    assert_eq!(listed[0]["structured"], true);
    let read = run(
        fixture.path(),
        &["--json", "memory", "phase", "read", "local/v1", "0"],
        b"",
    );
    let read: Value = serde_json::from_slice(&read.stdout).expect("phase read");
    assert_eq!(read["content"], phase());

    let current = memory.read().expect("current main");
    let unchanged = memory
        .settle(&current.revision, phase(), &current.content)
        .expect_err("unchanged main");
    assert_eq!(unchanged.code(), "memory.settle_unchanged");
    assert!(!memory.root().join("phases/PHASE-01.md").exists());
}
