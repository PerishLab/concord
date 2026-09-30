#[cfg(unix)]
mod handoff;
#[cfg(unix)]
mod provider;
#[cfg(unix)]
mod refresh;
#[cfg(unix)]
mod relations;

#[cfg(unix)]
mod unix {
    use super::super::execution::unix::{self as execution, Facts, git, repository, text};
    use super::super::spawn;
    use super::provider::{consume, observer, projection, start, tool};
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
        let provider = tool(fixture.path(), "provider", projection(fixture.path()));
        let provider = provider.to_str().expect("provider path");
        success(
            fixture.path(),
            &[
                "issue",
                "attach",
                "PerishLab/probe#1",
                "--github-command",
                provider,
            ],
        );
        let source = repository(fixture.path(), "PerishLab/probe");
        let observer = observer(fixture.path());
        success(
            fixture.path(),
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
        success(
            fixture.path(),
            &[
                "member",
                "start",
                "PerishLab/probe#1",
                "--claim",
                "topic.md",
                "--revision",
                "0",
                "--github-command",
                execution::provider(
                    fixture.path(),
                    Facts {
                        node: "I_delivery",
                        stable: "R_probe",
                        coordinate: "PerishLab/probe",
                        number: 1,
                    },
                )
                .to_str()
                .expect("start provider"),
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
        let member = fixture.path().join(".issues/I_delivery/worktree");
        std::fs::write(member.join("topic.md"), "delivery\n").expect("member delta");
        git(&member, &["add", "topic.md"]);
        git(&member, &["commit", "-m", "delivery"]);
        guard(&member, "PerishLab/probe");
        success(
            fixture.path(),
            &["member", "prove", "PerishLab/probe#1", "--revision", "1"],
        );
        super::handoff::verify(fixture.path(), provider);
        super::relations::verify(fixture.path(), provider);
        assert_eq!(
            success(fixture.path(), &["issue", "show", "PerishLab/probe#1"])["anchor"]["revision"],
            2
        );
        let plan = prepare(fixture.path(), provider, "2", true)
            .output()
            .expect("prepare stored plan");
        assert!(plan.status.success());
        std::fs::write(fixture.path().join("fail-create"), "one\n").expect("fail create");
        refused(
            &consume(fixture.path(), provider, &plan.stdout),
            "concord.delivery.provider",
        );
        assert_eq!(
            success(fixture.path(), &["issue", "show", "PerishLab/probe#1"])["anchor"]["revision"],
            2
        );
        std::fs::write(fixture.path().join("incomplete"), "one\n").expect("incomplete mode");
        refused(
            &consume(fixture.path(), provider, &plan.stdout),
            "concord.delivery.incomplete",
        );
        assert_eq!(
            success(fixture.path(), &["issue", "show", "PerishLab/probe#1"])["anchor"]["revision"],
            3
        );
        std::fs::remove_file(fixture.path().join("incomplete")).expect("enable merge");
        std::fs::write(fixture.path().join("hide-readback"), "one\n").expect("hide readback");
        refused(
            &consume(fixture.path(), provider, &plan.stdout),
            "concord.delivery.provider",
        );
        assert!(member.is_dir());
        std::fs::write(fixture.path().join("slow-readback"), "one\n").expect("slow readback");
        let first = start(fixture.path(), provider, &plan.stdout);
        let second = start(fixture.path(), provider, &plan.stdout);
        let outputs = [
            first.wait_with_output().expect("first land"),
            second.wait_with_output().expect("second land"),
        ];
        let mut reports = Vec::new();
        let mut refusals = Vec::new();
        for completed in outputs {
            if completed.status.success() {
                reports.push(
                    serde_json::from_slice::<Value>(&completed.stdout).expect("completion report"),
                );
            } else {
                refusals.push(
                    serde_json::from_slice::<Value>(&completed.stderr).expect("concurrent refusal"),
                );
            }
        }
        assert_eq!(reports.len(), 1);
        assert_eq!(refusals.len(), 1);
        assert_eq!(refusals[0]["error"]["code"], "concord.audit.refused");
        assert!(reports.iter().all(|report| {
            report["schema"] == "concord.issue-delivery-land/v3"
                && report["revision"] == 4
                && report["pull"]["state"] == "MERGED"
        }));
        assert_eq!(reports[0]["released"], true);
        assert!(!member.exists());
        assert_eq!(
            text(&source, &["rev-parse", "HEAD"]),
            reports[0]["integration"]["head"]
                .as_str()
                .expect("main head")
        );
        let replay = consume(fixture.path(), provider, &plan.stdout);
        assert!(replay.status.success());
        let replay: Value = serde_json::from_slice(&replay.stdout).expect("replay report");
        assert_eq!(replay["revision"], 4);
        assert_eq!(replay["released"], false);
    }

    pub(super) fn prepare(space: &Path, provider: &str, revision: &str, handoff: bool) -> Command {
        let mut command = spawn::concord(space);
        command
            .args(["--root", space.to_str().expect("root"), "--json"])
            .args([
                "issue",
                "delivery",
                "prepare",
                "PerishLab/probe#1",
                "--revision",
                revision,
                "--github-command",
                provider,
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
        let mut proof = Descriptor {
            schema: plumb::guard::SCHEMA.into(),
            repository: repository.into(),
            tree,
            plumb: authority.producer().into(),
            depot: authority.depot().into(),
            platform: plumb::config::platform(),
            actions: vec![Action {
                name: "guard/test".into(),
                input: "3".repeat(64),
                world: "4".repeat(64),
            }],
            digest: String::new(),
        };
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
        format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&claim).expect("claim"))
        )
    }

    pub(super) fn success(space: &Path, arguments: &[&str]) -> Value {
        let output = spawn::concord(space)
            .args(["--root", space.to_str().expect("root"), "--json"])
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
