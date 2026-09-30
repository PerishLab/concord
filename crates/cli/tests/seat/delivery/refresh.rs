use super::super::execution::unix::{self as execution, Facts, git, repository, text};
use super::provider::{consume, observer, projection, tool};
use super::unix::{guard, prepare, refused, success};
use serde_json::Value;
use std::path::Path;
use std::process::Output;

#[test]
fn refresh() {
    let fixture = tempfile::tempdir().expect("fixture");
    let seat = Fixture::open(fixture.path());
    seat.change("first\n");
    let first = seat.plan();
    seat.write("incomplete", "one\n");
    refused(&seat.land(&first), "concord.delivery.incomplete");
    assert_eq!(seat.read("pull-state"), "OPEN");
    seat.change("second\n");
    let second = seat.plan();
    seat.stale();
    refused(&seat.land(&second), "concord.delivery.incomplete");
    seat.current(&second);
    seat.detach();
    seat.change("third\n");
    let third = seat.plan();
    seat.stale();
    refused(&seat.land(&third), "concord.delivery.incomplete");
    seat.current(&third);
    seat.detach();
    seat.write("pull-state", "CLOSED");
    std::fs::remove_file(fixture.path().join("incomplete")).expect("enable merge");
    let fourth = seat.plan();
    let landed = seat.land(&fourth);
    assert!(
        landed.status.success(),
        "{}",
        String::from_utf8_lossy(&landed.stderr)
    );
    let report: Value = serde_json::from_slice(&landed.stdout).expect("land report");
    assert_eq!(report["pull"]["state"], "MERGED");
    assert_eq!(report["candidate"], delivery(&fourth)["candidate"]);
    assert_eq!(report["pull"]["headRefOid"], delivery(&fourth)["candidate"]);
    seat.write("pull-title", "tampered");
    refused(&seat.land(&fourth), "concord.delivery.stale");
    assert_eq!(seat.read("pull-title"), "tampered");
    assert_eq!(seat.read("pull-state"), "MERGED");
}

struct Fixture<'a> {
    space: &'a Path,
    provider: String,
}

impl<'a> Fixture<'a> {
    fn open(space: &'a Path) -> Self {
        success(space, &["issue", "bootstrap"]);
        let provider = tool(space, "provider", projection(space));
        let command = provider.to_str().expect("provider path");
        success(
            space,
            &[
                "issue",
                "attach",
                "PerishLab/probe#1",
                "--github-command",
                command,
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
        let provider = provider.to_str().expect("provider path").to_string();
        Self { space, provider }
    }

    fn change(&self, content: &str) {
        let member = self.space.join(".issues/I_delivery/worktree");
        std::fs::write(member.join("topic.md"), content).expect("member delta");
        git(&member, &["add", "topic.md"]);
        git(&member, &["commit", "-m", "delivery"]);
        guard(&member, "PerishLab/probe");
        let revision = self.revision();
        success(
            self.space,
            &[
                "member",
                "prove",
                "PerishLab/probe#1",
                "--revision",
                &revision,
            ],
        );
    }

    fn plan(&self) -> Vec<u8> {
        let output = prepare(self.space, &self.provider, &self.revision(), true)
            .output()
            .expect("prepare plan");
        assert!(output.status.success());
        let handoff: Value = serde_json::from_slice(&output.stdout).expect("handoff JSON");
        assert!(handoff["plan"].is_object(), "{handoff}");
        output.stdout
    }

    fn land(&self, plan: &[u8]) -> Output {
        consume(self.space, &self.provider, plan)
    }

    fn stale(&self) {
        self.write("pull-title", "stale title");
        self.write("pull-body", "stale body");
    }

    fn current(&self, plan: &[u8]) {
        let delivery = delivery(plan);
        assert_eq!(self.read("pull-state"), "OPEN");
        assert_eq!(self.read("pull-title"), delivery["pull"]["title"]);
        assert_eq!(self.read("pull-body"), delivery["pull"]["body"]);
        let branch = format!("refs/heads/{}", self.read("pull-head"));
        let head = text(&self.space.join("remote.git"), &["rev-parse", &branch]);
        assert_eq!(head, delivery["candidate"]);
    }

    fn detach(&self) {
        let revision = self.revision();
        success(
            self.space,
            &[
                "member",
                "reference",
                "remove",
                "PerishLab/probe#1",
                "--owner",
                "PerishLab",
                "--repository",
                "probe",
                "--number",
                "7",
                "--revision",
                &revision,
                "--apply",
            ],
        );
    }

    fn revision(&self) -> String {
        let shown = success(self.space, &["issue", "show", "PerishLab/probe#1"]);
        shown["anchor"]["revision"].to_string()
    }

    fn read(&self, name: &str) -> String {
        std::fs::read_to_string(self.space.join(name)).expect("provider state")
    }

    fn write(&self, name: &str, content: &str) {
        std::fs::write(self.space.join(name), content).expect("provider state");
    }
}

fn delivery(plan: &[u8]) -> Value {
    let handoff: Value = serde_json::from_slice(plan).expect("handoff JSON");
    handoff["plan"]["delivery"].clone()
}
