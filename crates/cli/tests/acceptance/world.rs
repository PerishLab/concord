use serde_json::{Value, json};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

pub(super) fn page() -> Value {
    let issue = json!({
        "id": "I_issue", "number": 83,
        "url": "https://github.com/PerishLab/concord/issues/83",
        "state": "OPEN", "kind": {"name": "Feature"},
        "body": "Current exact acceptance conditions.",
        "updated": "2026-09-30T00:00:00Z",
        "labels": connection(vec![], None, 0),
        "comments": connection(vec![], None, 0),
    });
    let repository = json!({"id": "R_concord", "issue": issue});
    json!({"data": {"repository": repository}})
}

pub(super) fn connection(nodes: Vec<Value>, cursor: Option<&str>, total: usize) -> Value {
    json!({"nodes": nodes, "total": total, "page": {"next": cursor.is_some(), "cursor": cursor}})
}

pub(super) fn entry(node: &str, body: &str, number: usize) -> Value {
    json!({"id": node, "body": body,
        "url": format!("https://github.com/PerishLab/concord/issues/83#issuecomment-{number}")})
}

pub(super) fn provider(scratch: &Path, replies: &[Value]) -> PathBuf {
    let mut script = format!(
        "#!/bin/sh\nset -eu\ncat > '{0}/request'\ncount=0\nif [ -f '{0}/calls' ]; then read count < '{0}/calls'; fi\ncount=$((count+1))\nprintf '%s\\n' \"$count\" > '{0}/calls'\ncp '{0}/request' \"{0}/request-$count\"\ncase \"$count\" in\n",
        scratch.display()
    );
    for (index, reply) in replies.iter().enumerate() {
        let path = scratch.join(format!("reply-{index}.json"));
        std::fs::write(&path, serde_json::to_vec(reply).unwrap()).unwrap();
        script.push_str(&format!("{}) cat '{}';;\n", index + 1, path.display()));
    }
    script.push_str("*) exit 91;;\nesac\n");
    let path = scratch.join("provider");
    std::fs::write(&path, script).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    path
}

pub(super) fn command(scratch: &Path, provider: &Path, operation: &str) -> Command {
    let mut command = crate::spawn::concord(scratch);
    command.args([
        "--root",
        scratch.to_str().unwrap(),
        "--json",
        "acceptance",
        operation,
    ]);
    command.args([
        "PerishLab/concord#83",
        "--github-command",
        provider.to_str().unwrap(),
    ]);
    command
}

pub(super) fn invoke(
    scratch: &Path,
    provider: &Path,
    operation: &str,
    input: Option<&Value>,
) -> Output {
    let mut command = command(scratch, provider, operation);
    if operation == "apply" {
        command.arg("--apply");
    }
    if input.is_some() {
        command.env("CODEX_THREAD_ID", "01a0ebb1-3075-7d03-bdfe-291998dd6cbe");
    }
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().unwrap();
    if let Some(input) = input {
        child
            .stdin
            .take()
            .unwrap()
            .write_all(&serde_json::to_vec(input).unwrap())
            .unwrap();
    } else {
        drop(child.stdin.take());
    }
    child.wait_with_output().unwrap()
}

pub(super) fn decoded(output: &Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

pub(super) fn refused(output: &Output, code: &str) {
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(code),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

pub(super) fn intent() -> Value {
    json!({"prose": "Verified bounded source promise.", "marker": {
        "purpose": "declaration", "target": "source", "promise": "Merge the checked source change."
    }})
}

pub(super) fn prepared(page: &Value, operation: &str, intent: &Value) -> Value {
    let scratch = tempfile::tempdir().unwrap();
    let provider = provider(scratch.path(), &[page.clone(), page.clone()]);
    decoded(&invoke(scratch.path(), &provider, operation, Some(intent)))
}

pub(super) fn published(plan: &Value) -> Value {
    let body = plan["plan"]["intent"]["body"].as_str().unwrap();
    let node = json!({
        "id": "intended", "url": "https://github.com/PerishLab/concord/issues/83#issuecomment-2",
        "body": body, "createdAt": "2026-10-07T00:00:00Z"
    });
    let addition = json!({"commentEdge": {"node": node}});
    json!({"data": {"addComment": addition}})
}

pub(super) fn retained(page: &Value, plan: &Value, labels: &[&str]) -> Value {
    let mut page = page.clone();
    let issue = &mut page["data"]["repository"]["issue"];
    let mut entries = issue["comments"]["nodes"].as_array().unwrap().clone();
    entries.push(entry(
        "intended",
        plan["plan"]["intent"]["body"].as_str().unwrap(),
        2,
    ));
    let count = entries.len();
    issue["comments"] = connection(entries, None, count);
    issue["labels"] = connection(
        labels.iter().map(|name| json!({"name": name})).collect(),
        None,
        labels.len(),
    );
    issue["updated"] = "2026-10-07T00:00:00Z".into();
    page
}

pub(super) fn catalog(target: &str) -> Value {
    let label = json!({
        "id": format!("L_{target}"), "name": format!("acceptance:{target}")
    });
    json!({"data": {"repository": {"id": "R_concord", "label": label}}})
}

pub(super) fn altered() -> Value {
    let alteration = json!({"labelable": {"id": "I_issue"}});
    json!({"data": {"alter": alteration}})
}

pub(super) fn request(path: &Path) -> Value {
    let bytes = std::fs::read(path).unwrap();
    serde_json::from_slice(&bytes).unwrap()
}
