use super::super::spawn;
use serde_json::Value;

#[test]
fn retired() {
    let scratch = tempfile::tempdir().expect("scratch");
    let root = scratch.path().join("unopened");
    let plan = scratch.path().join("old-plan.json");
    let payload = r#"{"version":1,"plan":{"schema":"concord.issue-member-landing/v3"}}"#;
    std::fs::write(&plan, payload).expect("old plan");
    for json in [false, true] {
        for verb in ["prepare", "ready"] {
            let mut command = spawn::concord(scratch.path());
            command
                .env("PATH", scratch.path().join("no-tools"))
                .arg("--root")
                .arg(&root)
                .arg("--config")
                .arg(scratch.path().join("unreadable.toml"));
            if json {
                command.arg("--json");
            }
            command.args(["member", "landing", verb, "PerishLab/concord#218"]);
            if verb == "prepare" {
                command.args([
                    "--revision",
                    "2",
                    "--guard-schema",
                    "old",
                    "--guard-tree",
                    "old",
                    "--guard-digest",
                    "old",
                ]);
            } else {
                command.arg("--plan").arg(&plan);
            }
            let output = command.output().expect("legacy handoff");
            assert!(!output.status.success());
            assert!(output.stdout.is_empty());
            let text = String::from_utf8(output.stderr).expect("diagnostic");
            let message = if json {
                let error: Value = serde_json::from_str(&text).expect("error JSON");
                assert_eq!(error["error"]["code"], "concord.landing.retired");
                error["error"]["message"]
                    .as_str()
                    .expect("message")
                    .to_owned()
            } else {
                assert!(
                    text.contains("member landing prepare/ready is retired"),
                    "{text}"
                );
                text
            };
            for expected in [
                "plumb land",
                "concord issue delivery prepare PerishLab/concord#218",
                "concord issue delivery land PerishLab/concord#218",
                "--github-command",
                "--revision",
            ] {
                assert!(message.contains(expected), "{message}");
            }
            assert!(!root.exists());
            assert_eq!(
                std::fs::read_to_string(&plan).expect("preserved plan"),
                payload
            );
        }
    }
}

#[test]
fn guidance() {
    let scratch = tempfile::tempdir().expect("scratch");
    for path in [vec![], vec!["prepare"], vec!["ready"]] {
        let output = spawn::concord(scratch.path())
            .args(["member", "landing"])
            .args(path)
            .arg("--help")
            .output()
            .expect("help");
        assert!(output.status.success());
        let text = String::from_utf8(output.stdout).expect("help text");
        assert!(text.contains("retired"), "{text}");
        assert!(text.contains("concord issue delivery prepare"), "{text}");
        assert!(text.contains("concord issue delivery land"), "{text}");
    }
}
