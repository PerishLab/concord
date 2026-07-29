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
    assert!(human.contains("resources: OK local/health"));
    assert!(human.contains("resource:evidence: OK"));
    assert!(human.contains("filesystem:"));
    assert!(human.contains("memory:"));
}
