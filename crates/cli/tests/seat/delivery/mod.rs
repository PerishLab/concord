#[cfg(unix)]
#[path = "calls/execution.rs"]
mod execution;
#[cfg(unix)]
mod fixture;
#[cfg(unix)]
#[path = "calls/handoff.rs"]
mod handoff;
#[cfg(unix)]
mod native;
#[cfg(unix)]
mod needs;
#[cfg(unix)]
#[path = "calls/observed.rs"]
mod observed;
#[cfg(unix)]
mod provider;
#[cfg(unix)]
mod refresh;
#[cfg(unix)]
mod relations;
#[cfg(unix)]
mod settled;
#[cfg(unix)]
mod unix {
    use super::super::execution::unix::{git, repository, text};
    use super::super::spawn::{self, Server};
    use super::provider::{consume, serve, start, tool};
    use plumb::guard::{Action, Authority, Descriptor};
    use serde::Serialize;
    use serde_json::Value;
    use sha2::{Digest as _, Sha256};
    use std::path::Path;
    use std::process::{Command, Stdio};
    #[derive(Serialize)]
    struct Claim<'a> {
        schema: &'a str,
        repository: &'a str,
        tree: &'a str,
        plumb: &'a str,
        depot: &'a str,
        platform: &'a str,
        actions: &'a [Action],
    }
    #[test]
    fn handoff() {
        let fixture = tempfile::tempdir().expect("fixture");
        success(fixture.path(), &["issue", "bootstrap"]);
        let server = serve(fixture.path(), "PerishLab/probe", "R_probe");
        attached(
            fixture.path(),
            &server,
            &["issue", "attach", "PerishLab/probe#1"],
        );
        let source = repository(fixture.path(), "PerishLab/probe");
        attached(
            fixture.path(),
            &server,
            &[
                "integration",
                "register",
                "PerishLab/probe",
                "--path",
                source.to_str().expect("source path"),
            ],
        );
        attached(
            fixture.path(),
            &server,
            &[
                "member",
                "start",
                "PerishLab/probe#1",
                "--claim",
                "topic.md",
                "--revision",
                "0",
            ],
        );
        git(
            &source,
            &[
                "config",
                "--unset-all",
                &format!(
                    "url.{}.insteadOf",
                    fixture.path().join("remote.git").display()
                ),
            ],
        );
        let ssh = tool(
            fixture.path(),
            "ssh",
            format!(
                "for last; do :; done\nexec sh -c \"$(printf '%s' \"$last\" | sed \"s|'/PerishLab/probe'|'{}'|\")\"",
                fixture.path().join("remote.git").display()
            ),
        );
        git(
            &source,
            &[
                "remote",
                "set-url",
                "origin",
                "ssh://git@github.com/PerishLab/probe",
            ],
        );
        git(
            &source,
            &["config", "core.sshCommand", ssh.to_str().expect("ssh")],
        );
        git(&source, &["config", "ssh.variant", "simple"]);
        let member = fixture.path().join("workspace/issues/I_delivery/worktree");
        std::fs::write(member.join("topic.md"), "delivery\n").expect("member delta");
        git(&member, &["add", "topic.md"]);
        git(&member, &["commit", "-m", "delivery"]);
        guard(&member, "PerishLab/probe");
        success(
            fixture.path(),
            &["member", "prove", "PerishLab/probe#1", "--revision", "1"],
        );
        super::handoff::verify(fixture.path(), &server);
        super::relations::verify(fixture.path(), &server);
        assert_eq!(
            success(fixture.path(), &["issue", "show", "PerishLab/probe#1"])["anchor"]["revision"],
            2
        );
        let plan = prepare(fixture.path(), &server, "2", true)
            .output()
            .expect("prepare stored plan");
        assert!(plan.status.success());
        std::fs::write(fixture.path().join("fail-create"), "one\n").expect("fail create");
        refused(
            &consume(fixture.path(), Some(&server), &plan.stdout),
            "concord.delivery.provider",
        );
        assert_eq!(
            success(fixture.path(), &["issue", "show", "PerishLab/probe#1"])["anchor"]["revision"],
            2
        );
        std::fs::write(fixture.path().join("incomplete"), "one\n").expect("incomplete mode");
        refused(
            &consume(fixture.path(), Some(&server), &plan.stdout),
            "concord.delivery.incomplete",
        );
        assert_eq!(
            success(fixture.path(), &["issue", "show", "PerishLab/probe#1"])["anchor"]["revision"],
            3
        );
        std::fs::remove_file(fixture.path().join("incomplete")).expect("enable merge");
        std::fs::write(fixture.path().join("hide-readback"), "one\n").expect("hide readback");
        refused(
            &consume(fixture.path(), Some(&server), &plan.stdout),
            "concord.delivery.provider",
        );
        assert!(member.is_dir());
        std::fs::write(fixture.path().join("slow-readback"), "one\n").expect("slow readback");
        let first = start(fixture.path(), Some(&server), &plan.stdout);
        let second = start(fixture.path(), Some(&server), &plan.stdout);
        let outputs = [
            first.wait_with_output().expect("first land"),
            second.wait_with_output().expect("second land"),
        ];
        let reports = super::execution::reports(&outputs);
        let released = reports
            .iter()
            .filter(|report| report["released"] == true)
            .collect::<Vec<_>>();
        assert_eq!(released.len(), 1);
        assert!(!member.exists());
        assert_eq!(
            text(&source, &["rev-parse", "HEAD"]),
            released[0]["integration"]["head"]
                .as_str()
                .expect("main head")
        );
        let replay = consume(fixture.path(), Some(&server), &plan.stdout);
        assert!(replay.status.success());
        let replay: Value = serde_json::from_slice(&replay.stdout).expect("replay report");
        assert_eq!(replay["revision"], 4);
        assert_eq!(replay["released"], false);
    }

    pub(super) fn prepare(space: &Path, server: &Server, revision: &str, handoff: bool) -> Command {
        let mut command = spawn::concord(space);
        server.attach(&mut command);
        command
            .args(["--home", space.to_str().expect("root"), "--json"])
            .args([
                "issue",
                "delivery",
                "prepare",
                "PerishLab/probe#1",
                "--revision",
                revision,
                "--observe-timeout",
                "1",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if handoff {
            command.arg("--handoff");
        }
        command
    }

    pub(super) fn refused(output: &std::process::Output, code: &str) {
        assert!(!output.status.success());
        let error: Value = serde_json::from_slice(&output.stderr).expect("land error");
        assert_eq!(error["error"]["code"], code);
        assert_ne!(error["error"]["code"], "concord.audit.refused");
    }

    pub(super) fn guard(member: &Path, repository: &str) {
        let authority = Authority::released().expect("compiled Plumb authority");
        let tree = text(member, &["rev-parse", "HEAD^{tree}"]);
        let mut proof: Descriptor = serde_json::from_value(serde_json::json!({
            "schema": plumb::guard::SCHEMA,
            "repository": repository,
            "tree": tree,
            "plumb": authority.producer(),
            "depot": authority.depot(),
            "platform": plumb::config::platform(),
            "actions": [{"name": "guard/test", "input": "3".repeat(64), "world": "4".repeat(64)}],
            "digest": ""
        }))
        .expect("legacy Guard proof fixture");
        proof.digest = digest(&proof);
        let token = proof.encode().expect("proof");
        git(
            member,
            &[
                "commit",
                "--amend",
                "-m",
                &format!("delivery\n\n{} {token}", plumb::guard::TRAILER),
            ],
        );
    }

    fn digest(proof: &Descriptor) -> String {
        let claim = Claim {
            schema: &proof.schema,
            repository: &proof.repository,
            tree: &proof.tree,
            plumb: &proof.plumb,
            depot: &proof.depot,
            platform: &proof.platform,
            actions: &proof.actions,
        };
        let bytes = serde_json::to_vec(&claim).expect("claim");
        format!("{:x}", Sha256::digest(bytes))
    }

    pub(super) fn attached(space: &Path, server: &Server, arguments: &[&str]) -> Value {
        let mut command = spawn::concord(space);
        server.attach(&mut command);
        let output = command
            .args(["--home", space.to_str().expect("root"), "--json"])
            .args(arguments)
            .output()
            .expect("run Concord");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).expect("Concord JSON")
    }

    pub(super) fn success(space: &Path, arguments: &[&str]) -> Value {
        let output = spawn::concord(space)
            .args(["--home", space.to_str().expect("root"), "--json"])
            .args(arguments)
            .output()
            .expect("run Concord");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).expect("Concord JSON")
    }
}
