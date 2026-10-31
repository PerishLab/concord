use super::provider::consume;
use super::unix::prepare;
use serde_json::{Value, json};
use std::path::Path;

pub fn verify(space: &Path, provider: &str) {
    let file = space.join("relationships");
    let first = relation(2);
    let second = relation(3);
    write(&file, &json!([second, first]));
    let plan = prepare(space, provider, "2", true)
        .output()
        .expect("prepare");
    assert!(
        plan.status.success(),
        "{}",
        String::from_utf8_lossy(&plan.stderr)
    );
    let value: Value = serde_json::from_slice(&plan.stdout).expect("plan");
    assert_eq!(
        value["plan"]["delivery"]["issue"]["blocking"][0]["number"],
        2
    );
    write(&file, &json!([first, second]));
    std::fs::write(space.join("fail-create"), "once").expect("stop after revalidation");
    refuses(
        &consume(space, provider, &plan.stdout),
        "concord.delivery.provider",
    );
    let mut opened = second.clone();
    opened["state"] = json!("OPEN");
    for changed in [
        json!([first]),
        json!([first, second, relation(4)]),
        json!([first, opened]),
    ] {
        write(&file, &changed);
        refuses(
            &consume(space, provider, &plan.stdout),
            "concord.delivery.stale",
        );
    }
    std::fs::remove_file(file).expect("restore empty relationships");
}

fn relation(number: u64) -> Value {
    json!({
        "id": format!("I_dependency_{number}"), "number": number,
        "url": format!("https://github.com/PerishLab/probe/issues/{number}"),
        "title": format!("dependency {number}"), "state": "CLOSED",
        "repository": {"nameWithOwner": "PerishLab/probe"},
    })
}

fn write(path: &Path, value: &Value) {
    std::fs::write(path, serde_json::to_vec(value).expect("relationships"))
        .expect("provider facts");
}

fn refuses(output: &std::process::Output, code: &str) {
    assert!(!output.status.success());
    let error: Value = serde_json::from_slice(&output.stderr).expect("refusal");
    assert_eq!(error["error"]["code"], code);
}

#[test]
fn pending() {
    for machine in [false, true] {
        let fixture = tempfile::tempdir().expect("fixture");
        let seat = Probe::open(fixture.path());
        let space = seat.space;
        let plan = seat.plan();
        std::fs::write(space.join("checks-pending"), "2").expect("pending checks");
        for remaining in ["1", "0"] {
            let pending = seat.poll(&plan, machine);
            assert_eq!(
                pending.status.code(),
                Some(3),
                "{}",
                String::from_utf8_lossy(&pending.stderr)
            );
            assert!(pending.stderr.is_empty());
            assert_eq!(
                std::fs::read_to_string(space.join("checks-pending")).unwrap(),
                remaining
            );
            assert!(!space.join("gate-marks").exists());
            assert!(!space.join("merge-attempts").exists());
            assert!(space.join(".issues/I_delivery/worktree").is_dir());
            if machine {
                let result: Value =
                    serde_json::from_slice(&pending.stdout).expect("pending result");
                assert_eq!(result["status"], "pending");
                assert_eq!(result["pull"]["number"], 7);
                assert_eq!(result["cookbook"], "concord.delivery.pending");
                assert!(result.get("error").is_none());
            } else {
                assert!(String::from_utf8_lossy(&pending.stdout).contains("pending"));
            }
        }
        let landed = seat.poll(&plan, true);
        assert!(
            landed.status.success(),
            "{}",
            String::from_utf8_lossy(&landed.stderr)
        );
        let result: Value = serde_json::from_slice(&landed.stdout).expect("land report");
        assert_eq!(result["released"], true);
        let prepared: Value = serde_json::from_slice(&plan).expect("plan");
        assert_eq!(
            result["candidate"],
            prepared["plan"]["delivery"]["candidate"]
        );
        assert!(!space.join(".issues/I_delivery/worktree").exists());
    }
}

#[test]
fn failures() {
    for mixed in [false, true] {
        let fixture = tempfile::tempdir().expect("fixture");
        let seat = Probe::open(fixture.path());
        let space = seat.space;
        let plan = seat.plan();
        let mut checks = vec![
            json!({"__typename":"StatusContext", "context":"broken", "state":"FAILURE", "targetUrl":"https://example.test/check"}),
        ];
        if mixed {
            checks.push(
                json!({"__typename":"StatusContext", "context":"waiting", "state":"PENDING"}),
            );
        }
        write(
            &space.join("checks-rollup"),
            &json!({"statusCheckRollup":checks}),
        );
        let failed = seat.poll(&plan, true);
        assert_eq!(failed.status.code(), Some(1));
        refuses(&failed, "concord.delivery.checks");
        assert!(String::from_utf8_lossy(&failed.stderr).contains("https://example.test/check"));
        assert!(!space.join("gate-marks").exists());
        assert!(!space.join("merge-attempts").exists());
    }
}

#[test]
fn drift() {
    use super::super::execution::unix::git;
    for (change, code) in [
        ("source", "concord.delivery.boundary"),
        ("base", "concord.delivery.guard"),
        ("acceptance", "concord.acceptance.changed"),
    ] {
        let fixture = tempfile::tempdir().expect("fixture");
        let seat = Probe::open(fixture.path());
        let space = seat.space;
        seat.declare("original");
        let plan = seat.plan();
        std::fs::write(space.join("checks-pending"), "2").expect("pending checks");
        assert_eq!(seat.poll(&plan, true).status.code(), Some(3));
        match change {
            "source" => {
                let member = space.join(".issues/I_delivery/worktree");
                std::fs::write(member.join("topic.md"), "changed").expect("source drift");
                git(&member, &["commit", "-am", "changed source"]);
            }
            "base" => {
                let source = space.join("source");
                std::fs::write(source.join("README.md"), "changed").expect("base drift");
                git(&source, &["commit", "-am", "changed base"]);
                git(&source, &["push", "origin", "main"]);
            }
            _ => seat.declare("expanded"),
        }
        let refused = seat.poll(&plan, true);
        assert_eq!(refused.status.code(), Some(1));
        refuses(&refused, code);
        assert!(!space.join("gate-marks").exists());
        assert!(!space.join("merge-attempts").exists());
        assert!(space.join(".issues/I_delivery/worktree").is_dir());
    }
}

struct Probe<'a> {
    space: &'a Path,
    provider: String,
}

impl<'a> Probe<'a> {
    fn open(space: &'a Path) -> Self {
        Self {
            space,
            provider: super::settled::open(space),
        }
    }
    fn declare(&self, promise: &str) {
        use concord_core::acceptance::{Marker, Target};
        let space = self.space;
        write(
            &space.join("labels"),
            &json!({"nodes":[{"name":"acceptance:source"}], "pageInfo":{"hasNextPage":false}}),
        );
        let declaration = Marker::Declaration {
            target: Target::Source,
            promise: promise.into(),
        }
        .render()
        .expect("declaration");
        std::fs::write(space.join("declaration"), declaration).expect("acceptance fixture");
    }

    fn plan(&self) -> Vec<u8> {
        let space = self.space;
        let provider = self.provider.as_str();
        let prepared = prepare(space, provider, &super::settled::revision(space), false)
            .output()
            .expect("prepare");
        assert!(
            prepared.status.success(),
            "{}",
            String::from_utf8_lossy(&prepared.stderr)
        );
        prepared.stdout
    }

    fn poll(&self, plan: &[u8], machine: bool) -> std::process::Output {
        let space = self.space;
        let provider = self.provider.as_str();
        let mut command = super::super::spawn::concord(space);
        command
            .args([
                "--space",
                space.to_str().expect("root"),
                "issue",
                "delivery",
                "land",
                "PerishLab/probe#1",
                "--no-wait",
                "--github-command",
                provider,
            ])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        if machine {
            command.arg("--json");
        }
        super::provider::feed(command, plan)
            .wait_with_output()
            .expect("land pass")
    }
}
