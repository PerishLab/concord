use super::super::spawn::{self, Server};
use super::provider::consume;
use super::unix::prepare;
use serde_json::Value;
use std::path::Path;

pub fn verify(space: &Path, server: &Server) {
    let direct = prepare(space, server, "1", false)
        .output()
        .expect("prepare direct refusal");
    refused(&direct, "concord.issue.stale");
    assert!(direct.stdout.is_empty());
    let refusal = prepare(space, server, "1", true)
        .output()
        .expect("prepare refusal handoff");
    assert!(refusal.status.success());
    assert!(refusal.stderr.is_empty());
    let expected: Value = serde_json::from_slice(&refusal.stdout).expect("refusal handoff JSON");
    carried(space, &refusal.stdout, &expected);
    for marker in ["provider-failure", "provider-delay"] {
        std::fs::write(space.join(marker), "one\n").expect("provider failure mode");
        let refusal = prepare(space, server, "2", true)
            .output()
            .expect("prepare provider refusal handoff");
        let _ = std::fs::remove_file(space.join(marker));
        assert!(refusal.status.success());
        assert!(refusal.stderr.is_empty());
        let expected: Value =
            serde_json::from_slice(&refusal.stdout).expect("provider refusal JSON");
        carried(space, &refusal.stdout, &expected);
    }
    refused(&consume(space, None, b""), "concord.delivery.handoff");
    let malformed = space.join("malformed-plan.json");
    std::fs::write(&malformed, b"").expect("malformed plan");
    let output = spawn::concord(space)
        .args(["--home", space.to_str().expect("root"), "--json"])
        .args(["issue", "delivery", "land", "PerishLab/probe#1"])
        .args(["--plan", malformed.to_str().expect("plan path")])
        .output()
        .expect("consume malformed plan");
    refused(&output, "concord.input.json");
}

fn carried(space: &Path, handoff: &[u8], expected: &Value) {
    let output = consume(space, None, handoff);
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

#[test]
fn adaptor() {
    for legacy in [false, true] {
        let fixture = tempfile::tempdir().expect("fixture");
        let space = fixture.path();
        let server = super::settled::open(space);
        let mut command = delivery(space, "prepare");
        server.attach(&mut command);
        command
            .args(["--revision", &super::settled::revision(space)])
            .env("GH_HOST", "unsupported.example");
        let prepared = command.output().expect("prepare");
        assert!(
            prepared.status.success(),
            "{}",
            String::from_utf8_lossy(&prepared.stderr)
        );
        let mut plan: Value = serde_json::from_slice(&prepared.stdout).expect("plan");
        assert_eq!(plan["version"], 2);
        assert_eq!(plan["forge"]["kind"], "github");
        assert_eq!(plan["forge"]["host"], "github.com");
        assert_eq!(plan["forge"]["node"], "R_probe");
        assert_eq!(plan["forge"]["repository"], "PerishLab/probe");
        for field in ["kind", "host", "repository", "node"] {
            let mut changed = plan.clone();
            changed["forge"][field] = "unsupported".into();
            refused(
                &consume(
                    space,
                    None,
                    &serde_json::to_vec(&changed).expect("changed plan"),
                ),
                "concord.delivery.adaptor",
            );
        }
        let unknown = delivery(space, "land")
            .args(["--forge", "forgejo"])
            .output()
            .expect("unsupported forge");
        refused(&unknown, "concord.delivery.adaptor");
        assert!(!space.join("pull-state").exists());
        let source = space.join("source");
        super::super::execution::unix::git(
            &source,
            &[
                "remote",
                "set-url",
                "origin",
                "https://forgejo.example/PerishLab/probe",
            ],
        );
        refused(
            &consume(space, None, &prepared.stdout),
            "concord.delivery.adaptor",
        );
        super::super::execution::unix::git(
            &source,
            &[
                "remote",
                "set-url",
                "origin",
                "ssh://git@github.com/PerishLab/probe",
            ],
        );
        if legacy {
            plan["version"] = 1.into();
            plan.as_object_mut().expect("envelope").remove("forge");
        }
        let mut command = delivery(space, "land");
        server.attach(&mut command);
        command.env("GH_HOST", "unsupported.example");
        let landed = super::provider::feed(command, &serde_json::to_vec(&plan).expect("plan"))
            .wait_with_output()
            .expect("land");
        assert!(
            landed.status.success(),
            "{}",
            String::from_utf8_lossy(&landed.stderr)
        );
        let landed: Value = serde_json::from_slice(&landed.stdout).expect("land report");
        assert_eq!(landed["released"], true);
        assert!(!space.join("workspace/issues/I_delivery/worktree").exists());
    }
}

#[test]
fn programs() {
    let fixture = tempfile::tempdir().expect("fixture");
    let space = fixture.path();
    let server = super::settled::open(space);
    let directory = space.join("bin");
    std::fs::create_dir(&directory).expect("tool directory");
    plumb::testing::executable(
        &directory.join("git"),
        "#!/bin/sh\nexec /usr/bin/git \"$@\"\n",
    );
    let blocked = directory.join("blocked");
    std::fs::write(&blocked, "no executable permission").expect("non executable");
    for token in [
        None,
        Some(blocked.as_path()),
        Some(Path::new("/missing/token")),
    ] {
        let mut command = delivery(space, "prepare");
        command
            .args(["--revision", &super::settled::revision(space)])
            .env("PATH", &directory);
        for credential in ["GH_TOKEN", "GITHUB_TOKEN", "GITHUB_TOKEN_FILE"] {
            command.env_remove(credential);
        }
        if let Some(path) = token {
            command.env("GITHUB_TOKEN_FILE", path);
        }
        let result = command.output().expect("missing credential refusal");
        let message = String::from_utf8_lossy(&result.stderr);
        refused(&result, "concord.issue.projection.provider");
        if token.is_none() {
            assert!(message.contains("GitHub token"), "{message}");
        }
    }
    drop(server);
    assert!(!space.join("pull-state").exists());
}

fn delivery(space: &Path, action: &str) -> std::process::Command {
    let mut command = spawn::concord(space);
    command
        .args(["--home", space.to_str().expect("root"), "--json"])
        .args(["issue", "delivery", action, "PerishLab/probe#1"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    command
}
