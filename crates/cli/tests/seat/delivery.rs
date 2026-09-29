#[cfg(unix)]
mod unix {
    use super::super::execution::unix::{git, repository, text};
    use super::super::spawn;
    use plumb::guard::{Action, Authority, Descriptor};
    use serde::Serialize;
    use serde_json::{Value, json};
    use sha2::{Digest as _, Sha256};
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
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
        let provider = tool(fixture.path(), projection());
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
        git(
            &source,
            &[
                "remote",
                "set-url",
                "origin",
                fixture.path().join("remote.git").to_str().expect("remote"),
            ],
        );
        success(
            fixture.path(),
            &[
                "member",
                "attach",
                "PerishLab/probe#1",
                "--claim",
                "topic.md",
                "--revision",
                "0",
            ],
        );
        let member = fixture.path().join(".issues/I_delivery/worktree");
        std::fs::write(member.join("topic.md"), "delivery\n").expect("member delta");
        git(&member, &["add", "topic.md"]);
        git(&member, &["commit", "-m", "delivery"]);
        let repository = format!(
            "{}/remote",
            fixture
                .path()
                .file_name()
                .expect("fixture name")
                .to_string_lossy()
        );
        guard(&member, &repository);
        success(
            fixture.path(),
            &["member", "prove", "PerishLab/probe#1", "--revision", "1"],
        );

        for _ in 0..3 {
            let mut producer = prepare(fixture.path(), provider).spawn().expect("prepare");
            let stdout = producer.stdout.take().expect("prepare stdout");
            let consumer = land(fixture.path(), provider)
                .stdin(Stdio::from(stdout))
                .output()
                .expect("consume streaming plan");
            assert!(producer.wait().expect("complete prepare").success());
            refused(&consumer);
        }
        let plan = prepare(fixture.path(), provider)
            .output()
            .expect("prepare stored plan");
        assert!(plan.status.success());
        let mut consumer = land(fixture.path(), provider)
            .stdin(Stdio::piped())
            .spawn()
            .expect("consume stored plan");
        consumer
            .stdin
            .as_mut()
            .expect("land stdin")
            .write_all(&plan.stdout)
            .expect("write stored plan");
        refused(&consumer.wait_with_output().expect("complete land"));
    }

    fn prepare(space: &Path, provider: &str) -> Command {
        let mut command = spawn::concord(space);
        command
            .args(["--root", space.to_str().expect("root"), "--json"])
            .args([
                "issue",
                "delivery",
                "prepare",
                "PerishLab/probe#1",
                "--revision",
                "2",
                "--github-command",
                provider,
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
    }

    fn land(space: &Path, provider: &str) -> Command {
        let mut command = spawn::concord(space);
        command
            .args(["--root", space.to_str().expect("root"), "--json"])
            .args([
                "issue",
                "delivery",
                "land",
                "PerishLab/probe#1",
                "--plan",
                "-",
                "--github-command",
                provider,
            ])
            .stderr(Stdio::piped());
        command
    }

    fn refused(output: &std::process::Output) {
        assert!(!output.status.success());
        let error: Value = serde_json::from_slice(&output.stderr).expect("land error");
        assert_eq!(error["error"]["code"], "concord.delivery.provider");
        assert_ne!(error["error"]["code"], "concord.audit.refused");
    }

    fn observer(root: &Path) -> PathBuf {
        let reply = json!({
            "node": "R_probe",
            "coordinate": "PerishLab/probe",
            "branch": "main",
        });
        let path = root.join("repository-provider");
        std::fs::write(&path, format!("#!/bin/sh\nprintf '%s\\n' '{reply}'\n"))
            .expect("repository provider");
        let mut permissions = std::fs::metadata(&path).expect("metadata").permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&path, permissions).expect("provider mode");
        path
    }

    fn guard(member: &Path, repository: &str) {
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

    fn projection() -> String {
        let observed = json!({
            "node": "I_delivery", "stable": "R_probe", "number": 1,
            "url": "https://github.com/PerishLab/probe/issues/1",
            "state": "OPEN", "kind": "Task", "updated_at": "2026-09-29T00:00:00Z",
        });
        let issue = json!({
            "id": "I_delivery", "number": 1,
            "url": "https://github.com/PerishLab/probe/issues/1",
            "title": "Stream delivery plan", "body": "## Outcome\nPipe exact plan",
            "state": "OPEN", "updatedAt": "2026-09-29T00:00:00Z",
            "issueType": {"name": "Task"},
            "repository": {"nameWithOwner": "PerishLab/probe"}, "parent": null,
            "subIssues": connection(),
            "subIssuesSummary": {"total": 0, "completed": 0, "percentCompleted": 0},
            "blockedBy": connection(), "blocking": connection(),
            "timelineItems": connection(), "comments": connection(),
        });
        let projected = json!({"data": {"repository": {"issue": issue}}});
        format!(
            "for argument in \"$@\"; do [ \"$argument\" = \"--jq\" ] && printf '%s\\n' '{observed}' && exit 0; done\nprintf '%s\\n' '{projected}'"
        )
    }

    fn connection() -> Value {
        json!({
            "totalCount": 0,
            "pageInfo": {"hasNextPage": false, "endCursor": null},
            "nodes": [],
        })
    }

    fn tool(root: &Path, body: String) -> PathBuf {
        let path = root.join("provider");
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("provider");
        let mut permissions = std::fs::metadata(&path).expect("metadata").permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&path, permissions).expect("provider mode");
        path
    }

    fn success(space: &Path, arguments: &[&str]) -> Value {
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
