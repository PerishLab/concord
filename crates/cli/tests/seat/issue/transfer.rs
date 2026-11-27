use super::super::execution::unix::{Facts, ticket};
use super::super::spawn::{self, Reply, Request, Server};
use serde_json::{Value, json};
use std::path::Path;
use std::process::Output;
use std::sync::Mutex;

const SOURCE: &str = "PerishLab/concord#25";
const TARGET: &str = "PerishLab/plumb#40";
const URL: &str = "https://github.com/PerishLab/plumb/issues/40";
const COMMENT: &str = "https://github.com/PerishLab/plumb/issues/40#issuecomment-1";

#[test]
fn recovery() {
    let fixture = tempfile::tempdir().expect("transfer fixture");
    let path = fixture.path();
    let server = server(true, true);
    attach(path, &server);

    let refused = transfer(path, &server, &["--apply"]);
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("comment.indeterminate"));

    let recovered = transfer(path, &server, &["--recover"]);
    assert_eq!(value(recovered)["status"], "recorded-source-retained");

    let resumed = transfer(path, &server, &["--apply"]);
    assert_eq!(value(resumed)["destination"]["node"], "I_new");

    let recovered = transfer(path, &server, &["--recover"]);
    let result = value(recovered);
    assert_eq!(result["status"], "recorded-and-source-absent");
    assert_eq!(result["record"], COMMENT);
}

#[test]
fn refusal() {
    let fixture = tempfile::tempdir().expect("transfer fixture");
    let path = fixture.path();
    let unresolved = server(false, false);
    attach(path, &unresolved);
    let refused = transfer(path, &unresolved, &["--apply"]);
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("exact source"));

    let qualified = server(true, false);
    let root = path.join("workspace/issues/I_old");
    std::fs::create_dir_all(root.join("artifacts/retained")).expect("unsettled payload");
    let refused = transfer(path, &qualified, &["--apply"]);
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("concord.issue.transfer.conflict"));
    assert_eq!(
        value(transfer(path, &qualified, &["--recover"]))["status"],
        "record-unobserved",
    );
}

fn attach(path: &Path, server: &Server) {
    value(run(path, server, &["issue", "bootstrap"]));
    value(run(path, server, &["issue", "attach", SOURCE]));
}

fn transfer(path: &Path, server: &Server, flags: &[&str]) -> Output {
    let mut arguments = vec![
        "issue",
        "transfer",
        SOURCE,
        TARGET,
        "--source-node",
        "I_old",
        "--destination-node",
        "I_new",
    ];
    arguments.extend_from_slice(flags);
    run(path, server, &arguments)
}

fn run(path: &Path, server: &Server, arguments: &[&str]) -> Output {
    server
        .attach(&mut spawn::concord(path))
        .args(["--home", path.to_str().expect("fixture path"), "--json"])
        .args(arguments)
        .output()
        .expect("run Concord")
}

fn value(output: Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("Concord JSON")
}

fn server(moved: bool, unknown: bool) -> Server {
    let record = Mutex::new(None);
    Server::start(move |request: Request| match request.path.as_str() {
        "/repos/PerishLab/concord/issues/25" if moved => Reply::status(301, json!({})).header(
            "Location",
            "https://api.github.com/repos/PerishLab/plumb/issues/40",
        ),
        "/repos/PerishLab/concord/issues/25" => Reply::status(301, json!({})).header(
            "Location",
            "https://api.github.com/repos/PerishLab/plumb/issues/41",
        ),
        "/repos/PerishLab/plumb/issues/40" => Reply::json(json!({
            "node_id":"I_new", "number":40, "html_url":URL,
            "repository_url":"https://api.github.com/repos/PerishLab/plumb",
            "title":"Transferred work", "body":"", "updated_at":"one",
            "state":"open", "state_reason":null, "type":{"name":"Feature"}, "user":null, "labels":[]
        })),
        _ if request.query().contains("addComment") => {
            assert!(unknown, "unexpected GitHub mutation");
            let mut record = record.lock().expect("record lock");
            assert!(record.is_none(), "attempted to replay GitHub comment");
            *record = Some(
                json!({"id":"IC_new","body":request.variables()["body"],"url":COMMENT,"created":"one","updated":"one"}),
            );
            Reply::status(500, json!({}))
        }
        _ if request.query().contains("RunsealTimeline") => {
            let rows = record
                .lock()
                .expect("record lock")
                .iter()
                .cloned()
                .collect::<Vec<_>>();
            timeline(&rows)
        }
        _ => Reply::json(ticket(
            &Facts {
                node: "I_old",
                stable: "R_concord",
                coordinate: "PerishLab/concord",
                number: 25,
            },
            "OPEN",
            "Feature",
            "one",
        )),
    })
}

fn timeline(rows: &[Value]) -> Reply {
    let page = json!({"more":false,"cursor":null});
    let comments = json!({"total":rows.len(),"rows":rows,"page":page});
    let issue = json!({"id":"I_new","number":40,"list":comments});
    Reply::json(json!({"data":{"repository":{"nameWithOwner":"PerishLab/plumb","issue":issue}}}))
}
