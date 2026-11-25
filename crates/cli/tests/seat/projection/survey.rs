#![cfg(unix)]
use super::super::execution::unix::repository;
use super::super::spawn::{self, Reply, Server};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

struct Span {
    trace: String,
    start: u64,
    end: u64,
}

#[test]
fn survey() {
    let fixture = tempfile::tempdir().expect("fixture");
    let space = fixture.path();
    success(space, None, &["issue", "bootstrap"]);
    for (node, coordinate) in [
        ("R_concord", "PerishLab/concord"),
        ("R_probe", "PerishLab/probe"),
    ] {
        register(space, node, coordinate);
    }
    let atoms = observe(space);
    let survey = atoms
        .iter()
        .find(|atom| edge(atom, "survey", "enter"))
        .expect("survey span");
    let texts = atoms
        .iter()
        .filter(|atom| atom["source"]["function"] == "text")
        .collect::<Vec<_>>();
    let forked = texts
        .iter()
        .filter(|atom| atom["context"]["locus.parent"] == survey["context"]["locus.span"])
        .map(|atom| atom["context"]["locus.trace"].to_string())
        .collect::<BTreeSet<_>>();
    assert_eq!(forked.len(), 2, "{forked:?}");
    assert!(!forked.contains(&survey["context"]["locus.trace"].to_string()));
    let spans = spans(&texts);
    for (index, span) in spans.iter().enumerate() {
        for other in &spans[index + 1..] {
            assert!(!tangled(span, other), "tangled spans on {}", span.trace);
        }
    }
}

fn tangled(left: &Span, right: &Span) -> bool {
    left.trace == right.trace && (crosses(left, right) || crosses(right, left))
}

fn crosses(outer: &Span, inner: &Span) -> bool {
    let starts = outer.start < inner.start && inner.start < outer.end;
    starts && outer.end < inner.end
}

fn spans(texts: &[&Value]) -> Vec<Span> {
    let mut spans = BTreeMap::new();
    for atom in texts {
        let span = atom["context"]["locus.span"].to_string();
        let at = atom["at"].as_u64().expect("at");
        let entry = spans.entry(span).or_insert(Span {
            trace: atom["context"]["locus.trace"].to_string(),
            start: at,
            end: at,
        });
        if edge(atom, "text", "return") {
            entry.end = at;
        }
    }
    spans.into_values().collect()
}

fn edge(atom: &Value, function: &str, edge: &str) -> bool {
    atom["source"]["function"] == function && atom["source"]["edge"] == edge
}

fn register(space: &Path, node: &str, coordinate: &str) {
    let root = space.join(node);
    std::fs::create_dir(&root).expect("repository root");
    let source = repository(&root, coordinate);
    let reply = json!({
        "id": 1,
        "node_id": node,
        "full_name": coordinate,
        "private": true,
        "default_branch": "main"
    });
    let server = Server::start(move |_| Reply::json(reply.clone()));
    success(
        space,
        Some(&server),
        &[
            "integration",
            "register",
            coordinate,
            "--path",
            source.to_str().expect("source"),
        ],
    );
}

fn observe(space: &Path) -> Vec<Value> {
    let config = space.join("home").join(".concord");
    std::fs::create_dir_all(&config).expect("config home");
    let closed = std::net::TcpListener::bind("127.0.0.1:0").expect("listen");
    let endpoint = format!("http://{}", closed.local_addr().expect("address"));
    drop(closed);
    std::fs::write(
        config.join("concord.toml"),
        format!("[locus]\nenabled = true\nendpoint = \"{endpoint}\"\n"),
    )
    .expect("config");
    success(space, None, &["audit"]);
    std::fs::read_to_string(space.join("state").join("locus").join("active.jsonl"))
        .expect("Locus buffer")
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("Locus atom"))
        .filter(|atom| atom["context"]["concord.command"] == "audit")
        .collect()
}

fn success(space: &Path, server: Option<&Server>, arguments: &[&str]) -> Value {
    let mut command = spawn::concord(space);
    command
        .args(["--home", space.to_str().expect("root path"), "--json"])
        .args(arguments);
    if let Some(server) = server {
        server.attach(&mut command);
    }
    let output = command.output().expect("run Concord");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("Concord JSON")
}
