use super::super::execution::unix::{self as execution, Facts, git, repository};
use super::provider::{consume, observer, projection, tool};
use super::unix::{guard, prepare, refused, success};
use concord_core::acceptance::{Marker, Target};
use serde_json::{Value, json};
use std::path::Path;

#[test]
fn needs() {
    let fixture = tempfile::tempdir().expect("fixture");
    let seat = Seat::open(fixture.path());
    let space = seat.space;
    let provider = seat.provider.as_str();
    seat.change();
    seat.label(&["needs:revalidation", "kind:bug"], false);
    let output = prepare(space, provider, &seat.revision(), false)
        .output()
        .expect("prepare");
    refused(&output, "concord.issue.needs");
    let error: Value = serde_json::from_slice(&output.stderr).expect("refusal JSON");
    let message = error["error"]["message"].as_str().expect("message");
    assert!(message.contains("needs:revalidation"), "{message}");
    assert!(!message.contains("kind:bug"), "{message}");
    seat.label(&["needs:revalidation"], true);
    refused(
        &prepare(space, provider, &seat.revision(), false)
            .output()
            .expect("prepare"),
        "concord.delivery.snapshot",
    );
    seat.label(&["kind:bug"], false);
    let plan = prepare(space, provider, &seat.revision(), true)
        .output()
        .expect("prepare");
    assert!(plan.status.success());
    let handoff: Value = serde_json::from_slice(&plan.stdout).expect("handoff JSON");
    assert!(handoff["plan"].is_object(), "{handoff}");
    let before = seat.revision();
    seat.label(&["needs:revalidation", "needs:owner"], false);
    let output = consume(space, provider, &plan.stdout);
    refused(&output, "concord.issue.needs");
    let error: Value = serde_json::from_slice(&output.stderr).expect("refusal JSON");
    let message = error["error"]["message"].as_str().expect("message");
    assert!(message.contains("needs:owner"), "{message}");
    assert!(message.contains("needs:revalidation"), "{message}");
    assert!(!space.join("pull-state").exists());
    assert_eq!(seat.revision(), before);
    std::fs::remove_file(space.join("labels")).expect("clear labels");
    let landed = consume(space, provider, &plan.stdout);
    assert!(
        landed.status.success(),
        "{}",
        String::from_utf8_lossy(&landed.stderr)
    );
}

struct Seat<'a> {
    space: &'a Path,
    provider: String,
}

#[test]
fn acceptance() {
    for mode in ["source", "release", "edited", "removed", "orphan"] {
        let fixture = tempfile::tempdir().unwrap();
        let seat = Seat::open(fixture.path());
        seat.change();
        let target = if mode == "release" {
            Target::Release
        } else {
            Target::Source
        };
        seat.label(&[target.label()], false);
        let marker = Marker::Declaration {
            target,
            promise: "Merge first, then verify every promised endpoint".into(),
        }
        .render()
        .unwrap();
        std::fs::write(seat.space.join("declaration"), &marker).unwrap();
        let revision = seat.revision();
        let prepared = prepare(seat.space, &seat.provider, &revision, true)
            .output()
            .unwrap();
        assert!(
            prepared.status.success(),
            "{}",
            String::from_utf8_lossy(&prepared.stderr)
        );
        let plan: Value = serde_json::from_slice(&prepared.stdout).unwrap();
        let body = plan["plan"]["delivery"]["pull"]["body"].as_str().unwrap();
        assert!(body.starts_with("Refs PerishLab/probe#1"));
        assert!(body.contains(target.label()));
        assert!(body.contains("Remaining obligations"));
        assert!(body.contains("no current closure judgment"));
        assert!(body.contains("IC_decl"));
        assert!(!body.contains("Closes PerishLab/probe#1"));
        assert_eq!(
            plan["plan"]["acceptance"]["evaluation"]["closure"],
            Value::Null
        );
        match mode {
            "edited" => {
                let changed = Marker::Declaration {
                    target,
                    promise: "Expanded endpoint after preparation".into(),
                }
                .render()
                .unwrap();
                std::fs::write(seat.space.join("declaration"), changed).unwrap();
            }
            "removed" => {
                seat.label(&[], false);
                std::fs::remove_file(seat.space.join("declaration")).unwrap();
            }
            "orphan" => seat.label(&[], false),
            _ => {}
        }
        let landed = consume(seat.space, &seat.provider, &prepared.stdout);
        if matches!(mode, "edited" | "removed" | "orphan") {
            let code = if mode == "orphan" {
                "concord.acceptance.protocol"
            } else {
                "concord.acceptance.changed"
            };
            refused(&landed, code);
            assert!(!seat.space.join("pull-state").exists());
            assert_eq!(seat.revision(), revision);
        } else {
            assert!(
                landed.status.success(),
                "{}",
                String::from_utf8_lossy(&landed.stderr)
            );
            let delivery: Value = serde_json::from_slice(&landed.stdout).unwrap();
            assert_eq!(delivery["merged"], true);
            assert!(body.contains("verification: `none`"));
        }
    }
}

impl<'a> Seat<'a> {
    fn label(&self, names: &[&str], more: bool) {
        let nodes = names
            .iter()
            .map(|name| json!({"name": name}))
            .collect::<Vec<_>>();
        let labels = json!({"nodes": nodes, "pageInfo": {"hasNextPage": more}});
        std::fs::write(self.space.join("labels"), labels.to_string()).expect("labels");
    }

    fn open(space: &'a Path) -> Self {
        success(space, &["issue", "bootstrap"]);
        let provider = tool(space, "provider", projection(space));
        let command = provider.to_str().expect("provider path").to_string();
        success(
            space,
            &[
                "issue",
                "attach",
                "PerishLab/probe#1",
                "--github-command",
                &command,
            ],
        );
        let source = repository(space, "PerishLab/probe");
        let observer = observer(space);
        success(
            space,
            &[
                "integration",
                "register",
                "PerishLab/probe",
                "--path",
                source.to_str().expect("source path"),
                "--github-command",
                observer.to_str().expect("repository provider"),
            ],
        );
        let facts = Facts {
            node: "I_delivery",
            stable: "R_probe",
            coordinate: "PerishLab/probe",
            number: 1,
        };
        let start = execution::provider(space, facts);
        success(
            space,
            &[
                "member",
                "start",
                "PerishLab/probe#1",
                "--claim",
                "topic.md",
                "--revision",
                "0",
                "--github-command",
                start.to_str().expect("start provider"),
            ],
        );
        let remote = space.join("remote.git");
        let rewrite = format!("url.{}.insteadOf", remote.display());
        git(&source, &["config", "--unset-all", &rewrite]);
        let ssh = tool(
            space,
            "ssh",
            format!(
                "for last; do :; done\nexec sh -c \"$(printf '%s' \"$last\" | sed \"s|'/PerishLab/probe'|'{}'|\")\"",
                remote.display()
            ),
        );
        let origin = "ssh://git@github.com/PerishLab/probe";
        git(&source, &["remote", "set-url", "origin", origin]);
        git(
            &source,
            &["config", "core.sshCommand", ssh.to_str().expect("ssh")],
        );
        git(&source, &["config", "ssh.variant", "simple"]);
        Self {
            space,
            provider: command,
        }
    }

    fn change(&self) {
        let member = self.space.join(".issues/I_delivery/worktree");
        std::fs::write(member.join("topic.md"), "needs\n").expect("member delta");
        git(&member, &["add", "topic.md"]);
        git(&member, &["commit", "-m", "delivery"]);
        guard(&member, "PerishLab/probe");
        success(
            self.space,
            &[
                "member",
                "prove",
                "PerishLab/probe#1",
                "--revision",
                &self.revision(),
            ],
        );
    }

    fn revision(&self) -> String {
        let shown = success(self.space, &["issue", "show", "PerishLab/probe#1"]);
        shown["anchor"]["revision"].to_string()
    }
}
