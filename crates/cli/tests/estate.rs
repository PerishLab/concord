use serde_json::{Value, json};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

#[test]
fn estate() {
    let fixture = tempfile::tempdir().expect("fixture");
    success(fixture.path(), &["domain", "bootstrap", "local"]);
    success(fixture.path(), &["domain", "add", "other"]);
    let alpha = success(fixture.path(), &["task", "start", "local", "alpha"]);
    assert_eq!(alpha["task"]["revision"], 0);

    let current = pipe(
        fixture.path(),
        &["task", "change"],
        json!({
            "version": 1,
            "task": "local/alpha",
            "revision": 0,
            "edits": [{
                "op": "create",
                "fact": {"role": "goal", "body": "Expose the control plane"}
            }]
        }),
    );
    assert_eq!(current["current"]["task"]["revision"], 1);
    let settled = pipe(
        fixture.path(),
        &["phase", "settle"],
        json!({
            "version": 1,
            "task": "local/alpha",
            "revision": 1,
            "phase": [{"part": "outcome", "body": "Graph kernel is visible"}],
            "edits": [{
                "op": "create",
                "fact": {"role": "next", "body": "Audit the estate"}
            }]
        }),
    );
    assert_eq!(settled["settlement"]["current"]["task"]["revision"], 2);

    let linked = success(
        fixture.path(),
        &[
            "task",
            "dependency",
            "add",
            "local/alpha",
            "other/beta",
            "--weight",
            "required",
            "--revision",
            "0",
            "--create-target",
        ],
    );
    assert_eq!(linked["revision"], 1);
    let adjacency = success(fixture.path(), &["graph", "adjacency"]);
    assert_eq!(adjacency["revision"], 1);
    assert_eq!(adjacency["adjacency"][0]["task"], "local/alpha");
    assert_eq!(adjacency["adjacency"][0]["out"], json!(["other/beta"]));
    assert_eq!(adjacency["adjacency"][1]["in"], json!(["local/alpha"]));
    let paths = success(
        fixture.path(),
        &["graph", "path", "local/alpha", "other/beta", "--all"],
    );
    assert_eq!(paths["paths"], json!([["local/alpha", "other/beta"]]));
    let rooted = success(
        fixture.path(),
        &["graph", "export", "--from", "local/alpha", "--depth", "0"],
    );
    assert_eq!(rooted["nodes"].as_array().expect("rooted nodes").len(), 1);
    assert_eq!(rooted["dependencies"], json!([]));

    let source = fixture.path().join("evidence.txt");
    std::fs::write(&source, "proof\n").expect("Artifact source");
    let imported = success(
        fixture.path(),
        &[
            "artifact",
            "import",
            "local/alpha",
            "proof",
            "--source",
            source.to_str().expect("source path"),
        ],
    );
    assert_eq!(imported["artifact"]["name"], "proof");
    let audit = success(fixture.path(), &["audit"]);
    assert_eq!(audit["agreement"]["faults"], json!([]));
    assert!(
        audit["agreement"]["observations"]
            .as_array()
            .expect("audit observations")
            .iter()
            .any(|finding| finding["code"] == "dependency.cross_domain")
    );

    for removed in ["todo", "memory", "resource"] {
        let output = raw(fixture.path(), &[removed]);
        assert!(!output.status.success(), "{removed} must stay absent");
        assert!(String::from_utf8_lossy(&output.stderr).contains("unrecognized subcommand"));
    }
}

fn success(space: &Path, arguments: &[&str]) -> Value {
    let output = raw(space, arguments);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("Concord JSON")
}

fn pipe(seat: &Path, arguments: &[&str], body: Value) -> Value {
    let mut child = command(seat)
        .args(arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn Concord");
    child
        .stdin
        .take()
        .expect("Concord stdin")
        .write_all(body.to_string().as_bytes())
        .expect("write change-set");
    let output = child.wait_with_output().expect("wait for Concord");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("Concord JSON")
}

fn raw(space: &Path, arguments: &[&str]) -> Output {
    command(space)
        .args(arguments)
        .output()
        .expect("run Concord")
}

fn command(seat: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_concord"));
    command
        .args(["--root", seat.to_str().expect("root path"), "--json"])
        .env_remove("CONCORD_LOCUS_ENABLED");
    command
}
