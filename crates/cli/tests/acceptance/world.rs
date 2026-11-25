use crate::spawn::{Reply, Server};
use serde_json::{Value, json};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::sync::Mutex;

pub(super) fn page() -> Value {
    let issue = json!({
        "id": "I_issue", "number": 83, "title": "Acceptance",
        "url": "https://github.com/PerishLab/concord/issues/83",
        "state": "OPEN", "reason": null, "writable": true,
        "kind": {"name": "Feature"},
        "repository": {"id": "R_concord"},
        "body": "Current exact acceptance conditions.",
        "updated": "2026-09-30T00:00:00Z",
        "labels": connection(vec![], None, 0),
        "comments": connection(vec![], None, 0),
    });
    let repository =
        json!({"id": "R_concord", "nameWithOwner": "PerishLab/concord", "issue": issue});
    json!({"data": {"repository": repository}})
}

pub(super) fn connection(rows: Vec<Value>, cursor: Option<&str>, total: usize) -> Value {
    json!({"rows": rows, "total": total, "page": {"more": cursor.is_some(), "cursor": cursor}})
}

pub(super) fn entry(node: &str, body: &str, number: usize) -> Value {
    json!({"id": node, "body": body,
        "url": format!("https://github.com/PerishLab/concord/issues/83#issuecomment-{number}")})
}

pub(super) fn provider(scratch: &Path, replies: &[Value]) -> Server {
    let root = scratch.to_path_buf();
    let held = replies.to_vec();
    let count = Mutex::new(0usize);
    Server::start(move |request| {
        let mut index = count.lock().unwrap();
        *index += 1;
        let current = *index;
        std::fs::write(root.join("request"), &request.body).unwrap();
        std::fs::write(root.join(format!("request-{current}")), &request.body).unwrap();
        std::fs::write(root.join("calls"), format!("{current}\n")).unwrap();
        match held.get(current - 1) {
            Some(reply) => Reply::json(reply.clone()),
            None => Reply::status(500, json!({"message": "unexpected request"})),
        }
    })
}

pub(super) fn command(scratch: &Path, server: &Server, operation: &str) -> Command {
    let mut command = crate::spawn::concord(scratch);
    server.attach(&mut command);
    command.args([
        "--home",
        scratch.to_str().unwrap(),
        "--json",
        "acceptance",
        operation,
        "PerishLab/concord#83",
    ]);
    command
}

pub(super) fn invoke(
    scratch: &Path,
    server: &Server,
    operation: &str,
    input: Option<&Value>,
) -> Output {
    let mut command = command(scratch, server, operation);
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
        "body": body, "created": "2026-10-07T00:00:00Z", "updated": "2026-10-07T00:00:00Z"
    });
    let addition = json!({"subject": {"id": "I_issue"}, "commentEdge": {"node": node}});
    json!({"data": {"addComment": addition}})
}

pub(super) fn retained(page: &Value, plan: &Value, labels: &[&str]) -> Value {
    let mut page = page.clone();
    let issue = &mut page["data"]["repository"]["issue"];
    let mut entries = issue["comments"]["rows"].as_array().unwrap().clone();
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
    let alteration = json!({"labelable": {"__typename": "Issue", "id": "I_issue"}});
    json!({"data": {"alter": alteration}})
}

pub(super) fn request(path: &Path) -> Value {
    let bytes = std::fs::read(path).unwrap();
    serde_json::from_slice(&bytes).unwrap()
}
