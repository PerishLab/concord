use super::super::spawn;
use super::provider::consume;
use super::unix::prepare;
use serde_json::Value;
use std::path::Path;

pub fn verify(space: &Path, provider: &str) {
    let direct = prepare(space, provider, "1", false)
        .output()
        .expect("prepare direct refusal");
    refused(&direct, "concord.issue.stale");
    assert!(direct.stdout.is_empty());
    let refusal = prepare(space, provider, "1", true)
        .output()
        .expect("prepare refusal handoff");
    assert!(refusal.status.success());
    assert!(refusal.stderr.is_empty());
    let expected: Value = serde_json::from_slice(&refusal.stdout).expect("refusal handoff JSON");
    carried(space, &refusal.stdout, &expected);
    for marker in ["provider-failure", "provider-delay"] {
        std::fs::write(space.join(marker), "one\n").expect("provider failure mode");
        let refusal = prepare(space, provider, "2", true)
            .output()
            .expect("prepare provider refusal handoff");
        assert!(refusal.status.success());
        assert!(refusal.stderr.is_empty());
        let expected: Value =
            serde_json::from_slice(&refusal.stdout).expect("provider refusal JSON");
        carried(space, &refusal.stdout, &expected);
    }
    refused(
        &consume(space, "/missing/provider", b""),
        "concord.delivery.handoff",
    );
    let malformed = space.join("malformed-plan.json");
    std::fs::write(&malformed, b"").expect("malformed plan");
    let output = spawn::concord(space)
        .args(["--root", space.to_str().expect("root"), "--json"])
        .args(["issue", "delivery", "land", "PerishLab/probe#1"])
        .args(["--plan", malformed.to_str().expect("plan path")])
        .args(["--github-command", "/missing/provider"])
        .output()
        .expect("consume malformed plan");
    refused(&output, "concord.input.json");
}

fn carried(space: &Path, handoff: &[u8], expected: &Value) {
    let output = consume(space, "/missing/provider", handoff);
    let actual: Value = serde_json::from_slice(&output.stderr).expect("carried refusal");
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(actual["error"]["code"], expected["error"]["code"]);
    assert_eq!(actual["error"]["message"], expected["error"]["message"]);
    assert_eq!(actual["error"]["details"], expected["error"]["details"]);
}

fn refused(output: &std::process::Output, code: &str) {
    assert!(!output.status.success());
    let error: Value = serde_json::from_slice(&output.stderr).expect("delivery error");
    assert_eq!(error["error"]["code"], code);
}
