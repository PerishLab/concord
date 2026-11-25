use super::world::{World, decoded, page};
use crate::execution::unix::{Facts, admission, record, repository, ticket};
use crate::spawn::{Reply, Request, Server};
use serde_json::{Value, json};

#[test]
fn unqualified() {
    for mode in ["untyped", "provider", "paging"] {
        let world = World::new();
        world.change(|state| {
            state["mode"] = mode.into();
            if mode == "untyped" {
                state["issue"]["issueType"] = Value::Null;
            }
        });
        let result = world.observe("preflight");
        let report = &result["preflight"];
        assert_eq!(report["complete"], false, "{mode}: {report}");
        assert_eq!(report["issue"]["node"], "");
        let phase = &report["phase"];
        assert_eq!(phase["status"], "unavailable");
        assert!(
            phase["reason"]
                .as_str()
                .unwrap()
                .contains("upstream Issue identity")
        );
        let code = report["diagnostics"][0]["code"].as_str().unwrap();
        assert!(code.starts_with("concord.issue.projection."), "{code}");
        assert_eq!(world.state()["reads"], json!(["root"]));
        assert_eq!(world.writes(), 0);
        world.unbind();
        assert_eq!(
            world.observe("preflight")["preflight"]["phase"]["status"],
            "not-configured"
        );
        assert_eq!(world.state()["reads"], json!(["root", "root"]));
    }
}

#[test]
fn qualified() {
    for expected in ["unset", "set", "invalid", "unavailable"] {
        let world = World::new();
        world.change(|state| {
            configure(state, expected);
            if expected == "unavailable" {
                state["issue"]["issueFieldValues"]["pageInfo"]["hasNextPage"] = false.into();
                state["mode"] = "drift".into();
            }
        });
        let result = world.observe("preflight");
        assert_eq!(result["preflight"]["complete"], true);
        assert_eq!(result["preflight"]["phase"]["status"], expected);
        let reads = world.state()["reads"].clone();
        assert_eq!(
            reads,
            if expected == "invalid" {
                json!(["root", "schema"])
            } else {
                json!(["root", "schema", "values"])
            }
        );
        assert_eq!(world.writes(), 0);
    }
}

#[test]
fn incomplete() {
    let world = World::new();
    member(&world);
    for expected in ["unset", "set", "invalid", "unavailable"] {
        world.change(|state| {
            state["reads"] = json!([]);
            state["mode"] = "peer".into();
            state["field"]["options"][2]["id"] = "IFSSO_v".into();
            state["issue"]["issueFieldValues"] = page(vec![]);
            configure(state, expected);
        });
        let result = world.observe("preflight");
        let report = &result["preflight"];
        assert_eq!(report["complete"], false, "{report}");
        assert_eq!(report["issue"]["node"], "I_work");
        assert_eq!(report["phase"]["status"], expected, "{report}");
        assert_eq!(
            world.state()["reads"],
            if expected == "invalid" {
                json!(["root", "peer", "schema"])
            } else {
                json!(["root", "peer", "schema", "values"])
            }
        );
        assert_eq!(world.writes(), 0);
    }
}

fn member(world: &World) {
    let space = world.root.path();
    let source = repository(space, "PerishLab/concord");
    let server = Server::start(|request: Request| {
        if request.method == "GET" && request.path == "/repos/PerishLab/concord" {
            return Reply::json(record("R_repo", "PerishLab/concord"));
        }
        let facts = Facts {
            node: "I_work",
            stable: "R_repo",
            coordinate: "PerishLab/concord",
            number: 4,
        };
        if request.query().contains("RunsealAdmission") {
            return Reply::json(admission(&facts, "Feature"));
        }
        Reply::json(ticket(&facts, "OPEN", "Feature", "original"))
    });
    decoded(&world.attached(
        &server,
        &[
            "integration",
            "register",
            "PerishLab/concord",
            "--path",
            source.to_str().unwrap(),
        ],
    ));
    decoded(&world.attached(&server, &["issue", "attach", "PerishLab/concord#4"]));
    decoded(&world.attached(
        &server,
        &[
            "member",
            "start",
            "PerishLab/concord#4",
            "--revision",
            "0",
            "--claim",
            "topic.md",
        ],
    ));
}

fn configure(state: &mut Value, expected: &str) {
    match expected {
        "set" => state["issue"]["issueFieldValues"] = value(),
        "invalid" => state["field"]["options"][2]["id"] = "changed".into(),
        "unavailable" => {
            state["issue"]["issueFieldValues"]["pageInfo"]["hasNextPage"] = true.into()
        }
        _ => {}
    }
}

fn value() -> Value {
    page(vec![
        json!({"__typename":"IssueFieldSingleSelectValue","id":"V_phase","optionId":"IFSSO_v","name":"Verify","field":{"id":"IFSS_phase"}}),
    ])
}
