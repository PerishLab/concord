use super::fixture::Native;
use super::unix::{refused, success};
use serde_json::{Value, json};
use std::io::Write;
use std::path::Path;
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

struct Run(Option<Child>);

impl Run {
    fn start(mut command: Command, input: Option<&[u8]>) -> Self {
        let mut child = command.stdin(Stdio::piped()).spawn().expect("child");
        if let Some(input) = input {
            child.stdin.take().unwrap().write_all(input).expect("plan");
        }
        drop(child.stdin.take());
        Self(Some(child))
    }

    fn output(mut self) -> Output {
        until(|| {
            self.0
                .as_mut()
                .unwrap()
                .try_wait()
                .expect("status")
                .is_some()
        });
        self.0.take().unwrap().wait_with_output().expect("output")
    }
}

impl Drop for Run {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn until(mut ready: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while !ready() {
        assert!(Instant::now() < deadline, "bounded child progress");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn observe(seat: &Native) {
    let provider = seat.space.join("provider");
    let script = std::fs::read_to_string(&provider).unwrap();
    let added = format!(
        "printf '%s|%s|%s|%s\\n' \"${{LAND_CALLER:-setup}}\" \"$1\" \"$2\" \"$7\" >> '{}/calls'\n",
        seat.space.display()
    );
    plumb::testing::executable(
        &provider,
        script.replacen("#!/bin/sh\n", &format!("#!/bin/sh\n{added}"), 1),
    );
}

fn polls(space: &Path) -> usize {
    std::fs::read_to_string(space.join("calls"))
        .unwrap_or_default()
        .lines()
        .filter(|line| *line == "holder|pr|view|statusCheckRollup")
        .count()
}

#[test]
fn contenders() {
    let temp = tempfile::tempdir().unwrap();
    let mut seat = Native::open(temp.path(), "R_kgDOUesM0Q");
    seat.tools();
    let prepared = seat.prepare("wharf-native");
    assert!(prepared.status.success());
    observe(&seat);
    let pending = seat.space.join("checks-rollup");
    std::fs::write(&pending, json!({"statusCheckRollup":[{"__typename":"CheckRun","name":"Guard","status":"IN_PROGRESS","conclusion":null}]}).to_string()).unwrap();
    for name in ["plan-a", "plan-b"] {
        std::fs::write(seat.space.join(name), &prepared.stdout).unwrap();
    }
    let mut command = seat.command("land", "wharf-native");
    command
        .args(["--plan", seat.space.join("plan-a").to_str().unwrap()])
        .env("LAND_CALLER", "holder");
    let holder = Run::start(command, None);
    let pid = holder.0.as_ref().unwrap().id();
    until(|| polls(seat.space) >= 2);
    for path in ["plan-a", "plan-b", "-"] {
        let mut command = seat.command("land", "wharf-native");
        let file = seat.space.join(path);
        command
            .args([
                "--plan",
                if path == "-" {
                    "-"
                } else {
                    file.to_str().unwrap()
                },
            ])
            .env("LAND_CALLER", "contender");
        let output =
            Run::start(command, (path == "-").then_some(prepared.stdout.as_slice())).output();
        refused(&output, "concord.delivery.running");
        let error: Value = serde_json::from_slice(&output.stderr).unwrap();
        assert_eq!(error["error"]["details"]["holder"]["pid"], pid);
        assert_eq!(
            error["error"]["details"]["holder"]["plan"]
                .as_str()
                .unwrap()
                .len(),
            64
        );
    }
    assert!(
        !std::fs::read_to_string(seat.space.join("calls"))
            .unwrap()
            .contains("contender|")
    );
    assert!(
        success(seat.space, &["audit"])["agreement"]["faults"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    drop(holder);
    std::fs::remove_file(pending).unwrap();
    let resumed = seat.land("wharf-native", &prepared.stdout);
    assert!(
        resumed.status.success(),
        "{}",
        String::from_utf8_lossy(&resumed.stderr)
    );
    let report: Value = serde_json::from_slice(&resumed.stdout).unwrap();
    assert_eq!(report["released"], true);
    assert_eq!(
        std::fs::read_to_string(seat.space.join("merge-attempts")).unwrap(),
        "merge\n"
    );
    assert!(!seat.member.exists());
    let replay = seat.land("wharf-native", &prepared.stdout);
    assert!(replay.status.success());
    let replay: Value = serde_json::from_slice(&replay.stdout).unwrap();
    assert_eq!(replay["released"], false);
    assert_eq!(replay["merge"], report["merge"]);
}

#[test]
fn exits() {
    let temp = tempfile::tempdir().unwrap();
    let mut seat = Native::open(temp.path(), "R_kgDOUesM0Q");
    seat.tools();
    let prepared = seat.prepare("wharf-native");
    assert!(prepared.status.success());
    std::fs::write(seat.space.join("checks-pending"), "100").unwrap();
    let mut command = seat.command("land", "wharf-native");
    command.arg("--no-wait");
    let pending = Run::start(command, Some(&prepared.stdout)).output();
    assert_eq!(pending.status.code(), Some(3));
    std::fs::remove_file(seat.space.join("checks-pending")).unwrap();
    std::fs::write(seat.space.join("fail-gate"), "fail").unwrap();
    refused(
        &seat.land("wharf-native", &prepared.stdout),
        "concord.delivery.native",
    );
    std::fs::remove_file(seat.space.join("fail-gate")).unwrap();
    let resumed = seat.land("wharf-native", &prepared.stdout);
    assert!(
        resumed.status.success(),
        "{}",
        String::from_utf8_lossy(&resumed.stderr)
    );
}

pub(super) fn reports(outputs: &[Output]) -> Vec<Value> {
    let reports = outputs
        .iter()
        .filter(|completed| completed.status.success())
        .map(|completed| {
            serde_json::from_slice::<Value>(&completed.stdout).expect("completion report")
        })
        .collect::<Vec<_>>();
    assert_eq!(reports.len(), 1);
    let contender = outputs
        .iter()
        .find(|output| !output.status.success())
        .expect("contender refused");
    refused(contender, "concord.delivery.running");
    for report in &reports {
        assert_eq!(report["schema"], "concord.issue-delivery-land/v3");
        assert_eq!(report["revision"], 4);
        assert_eq!(report["pull"]["state"], "MERGED");
        assert_eq!(report["merge"], reports[0]["merge"]);
    }
    reports
}
