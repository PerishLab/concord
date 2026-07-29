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

#[test]
fn memory_write_and_settle_accept_one_explicit_stdin_source() {
    let fixture = tempfile::tempdir().expect("fixture");
    let root = Root::new(fixture.path()).expect("canonical root");
    let space = Space::new(root);
    space.domain_init("local", true).expect("initialize domain");
    space
        .task_start("local/memory", true)
        .expect("start memory task");
    let task = space.resolve("local/memory").expect("resolve task");
    let memory = Memory::new(&task);
    memory.init("# Initial\n").expect("initialize memory");
    let first = memory.read().expect("read initial memory");

    let written = run(
        fixture.path(),
        &[
            "--json",
            "memory",
            "write",
            "local/memory",
            "--expect",
            &first.revision,
            "--file",
            "-",
        ],
        b"# Written from stdin\n",
    );
    assert!(
        written.status.success(),
        "{}",
        String::from_utf8_lossy(&written.stderr)
    );
    let output: Value = serde_json::from_slice(&written.stdout).expect("write json");
    let revision = output["result"]["revision"]
        .as_str()
        .expect("written revision");
    assert_eq!(
        memory.read().expect("read written memory").content,
        "# Written from stdin\n"
    );

    let main = fixture.path().join("NEXT.md");
    std::fs::write(&main, "# Next\n").expect("write next main");
    let settled = run(
        fixture.path(),
        &[
            "--json",
            "memory",
            "settle",
            "local/memory",
            "--expect",
            revision,
            "--phase-file",
            "-",
            "--main-file",
            main.to_str().expect("main path"),
        ],
        b"# Settled from stdin\n",
    );
    assert!(
        settled.status.success(),
        "{}",
        String::from_utf8_lossy(&settled.stderr)
    );
    let output: Value = serde_json::from_slice(&settled.stdout).expect("settle json");
    let phase = output["result"]["phase"].as_str().expect("phase path");
    assert_eq!(
        std::fs::read_to_string(phase).expect("read phase"),
        "# Settled from stdin\n"
    );
    assert_eq!(
        memory.read().expect("read settled memory").content,
        "# Next\n"
    );

    let current = memory.read().expect("read next revision");
    let phase = fixture.path().join("PHASE.md");
    std::fs::write(&phase, "# Phase from file\n").expect("write phase");
    let settled = run(
        fixture.path(),
        &[
            "--json",
            "memory",
            "settle",
            "local/memory",
            "--expect",
            &current.revision,
            "--phase-file",
            phase.to_str().expect("phase path"),
            "--main-file",
            "-",
        ],
        b"# Main from stdin\n",
    );
    assert!(
        settled.status.success(),
        "{}",
        String::from_utf8_lossy(&settled.stderr)
    );
    let output: Value = serde_json::from_slice(&settled.stdout).expect("settle json");
    let phase = output["result"]["phase"].as_str().expect("phase path");
    assert_eq!(
        std::fs::read_to_string(phase).expect("read phase"),
        "# Phase from file\n"
    );
    assert_eq!(
        memory.read().expect("read stdin main").content,
        "# Main from stdin\n"
    );
}

#[test]
fn memory_stdin_preserves_cas_and_refuses_ambiguous_settle() {
    let fixture = tempfile::tempdir().expect("fixture");
    let root = Root::new(fixture.path()).expect("canonical root");
    let space = Space::new(root);
    space.domain_init("local", true).expect("initialize domain");
    space
        .task_start("local/memory", true)
        .expect("start memory task");
    let task = space.resolve("local/memory").expect("resolve task");
    let memory = Memory::new(&task);
    memory.init("# Initial\n").expect("initialize memory");
    let stale = memory.read().expect("read initial memory").revision;
    memory
        .write(&stale, "# Current\n")
        .expect("advance memory revision");

    let rejected = run(
        fixture.path(),
        &[
            "memory",
            "write",
            "local/memory",
            "--expect",
            &stale,
            "--file",
            "-",
        ],
        b"# Stale\n",
    );
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("memory revision changed"));
    assert_eq!(
        memory.read().expect("read current memory").content,
        "# Current\n"
    );

    let current = memory.read().expect("read current revision");
    let dry_run = run(
        fixture.path(),
        &[
            "memory",
            "write",
            "local/memory",
            "--expect",
            &current.revision,
            "--file",
            "-",
            "--dry-run",
        ],
        b"# Dry run\n",
    );
    assert!(dry_run.status.success());
    assert_eq!(
        memory.read().expect("read after dry run").revision,
        current.revision
    );

    let ambiguous = run(
        fixture.path(),
        &[
            "memory",
            "settle",
            "local/memory",
            "--expect",
            &current.revision,
            "--phase-file",
            "-",
            "--main-file",
            "-",
        ],
        b"",
    );
    assert!(!ambiguous.status.success());
    assert!(
        String::from_utf8_lossy(&ambiguous.stderr)
            .contains("memory settle accepts stdin for only one input")
    );
    assert_eq!(
        memory.read().expect("read unchanged memory").revision,
        current.revision
    );
}
