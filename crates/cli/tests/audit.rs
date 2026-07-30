use concord_core::{Memory, Root, Space};
use serde_json::Value;
use std::path::Path;
use std::process::{Command, Output};

fn run(root: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_concord"))
        .arg("--root")
        .arg(root)
        .args(arguments)
        .output()
        .expect("run concord")
}

#[test]
fn audit_keeps_agreement_and_resource_findings_separate() {
    let fixture = tempfile::tempdir().expect("fixture");
    let root = Root::new(fixture.path()).expect("canonical root");
    let space = Space::new(root);
    space.domain_init("local", true).expect("initialize domain");
    space.task_start("local/health", true).expect("start task");
    let task = space.resolve("local/health").expect("resolve task");
    let memory = Memory::new(&task);
    memory.init("# Health\n").expect("initialize memory");
    let source = fixture.path().join("evidence.txt");
    std::fs::write(&source, "small evidence\n").expect("write evidence");
    memory.import("evidence", &source).expect("import evidence");

    let output = run(fixture.path(), &["--json", "audit", "local/health"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout).expect("audit json");
    assert_eq!(report["faults"], serde_json::json!([]));
    assert_eq!(report["resources"][0]["identity"], "local/health");
    assert_eq!(report["resources"][0]["task"]["status"], "OK");
    assert_eq!(
        report["resources"][0]["resources"][0]["name"],
        "resource:evidence"
    );
    assert!(
        report["resources"][0]["filesystem"]["value"]["available_bytes"]
            .as_u64()
            .is_some_and(|value| value > 0)
    );
    assert!(
        report["resources"][0]["host_memory"]["value"]["available_bytes"]
            .as_u64()
            .is_some_and(|value| value > 0)
    );

    let human = run(fixture.path(), &["audit", "local/health"]);
    assert!(human.status.success());
    let human = String::from_utf8(human.stdout).expect("human audit");
    assert!(human.contains("agreement: true to the protocol"));
    assert!(human.contains("resources: "));
    assert!(human.contains("local/health"));
    assert!(human.contains("resource:evidence: OK"));
    assert!(human.contains("filesystem:"));
    assert!(human.contains("memory:"));
}

#[test]
fn memory_hygiene_and_phase_count_keep_distinct_audit_semantics() {
    let fixture = tempfile::tempdir().expect("fixture");
    let root = Root::new(fixture.path()).expect("canonical root");
    let space = Space::new(root);
    space.domain_init("local", true).expect("initialize domain");
    space.task_start("local/memory", true).expect("start task");
    let task = space.resolve("local/memory").expect("resolve task");
    let memory = Memory::new(&task);
    memory.init("# Legacy\n").expect("initialize memory");
    let phases = memory.root().join("phases");
    std::fs::create_dir(&phases).expect("phase directory");
    private_directory(&phases);
    for number in 0..16 {
        let path = phases.join(format!("PHASE-{number:02}.md"));
        std::fs::write(&path, format!("# Phase {number}\n")).expect("write phase");
        private_file(&path);
    }

    let audit = task.audit().expect("audit phases");
    assert!(audit.ok(), "phase-count guidance is not a fault");
    assert_eq!(audit.observations.len(), 1);
    assert!(audit.observations[0].message.contains("16 phases"));

    std::fs::write(
        memory.main(),
        "<!-- concord-memory:v1 -->\n## Goal\n\nIncomplete.\n",
    )
    .expect("break structured main");
    private_file(&memory.main());
    let audit = task.audit().expect("audit malformed memory");
    let fault = audit
        .faults
        .iter()
        .find(|fault| fault.kind == "memory")
        .expect("memory hygiene fault");
    assert!(!fault.gates());
    assert!(audit.agrees());
    assert!(!audit.ok());

    let output = run(fixture.path(), &["--json", "audit", "local/memory"]);
    assert!(
        !output.status.success(),
        "hygiene remains visible as nonzero"
    );
    let report: Value = serde_json::from_slice(&output.stdout).expect("audit JSON");
    assert_eq!(report["observations"][0]["kind"], "memory");
    assert!(
        report["faults"]
            .as_array()
            .expect("faults")
            .iter()
            .any(|fault| fault["kind"] == "memory")
    );
}

#[cfg(unix)]
fn private_directory(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
        .expect("private directory");
}

#[cfg(not(unix))]
fn private_directory(_: &Path) {}

#[cfg(unix)]
fn private_file(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).expect("private file");
}

#[cfg(not(unix))]
fn private_file(_: &Path) {}
