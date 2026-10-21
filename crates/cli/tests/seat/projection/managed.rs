use super::management::{Fixture, decoded, intent, refused};
use serde_json::json;

#[test]
fn configuration() {
    let fixture = Fixture::new("ordinary");
    let plan = decoded(&fixture.run(
        "prepare",
        &intent(json!({"kind":"configure","changes":{"title":"Reviewed planning"}})),
    ));
    assert!(fixture.state()["writes"].as_array().unwrap().is_empty());
    let result = decoded(&fixture.run("apply", &plan));
    assert_eq!(result["project"]["result"]["title"], "Reviewed planning");
    let state = fixture.state();
    assert_eq!(state["statusUpdates"].as_array().unwrap().len(), 2);
    assert!(
        state["statusUpdates"][0]["body"]
            .as_str()
            .unwrap()
            .contains("requested")
    );
    assert!(
        state["statusUpdates"][1]["body"]
            .as_str()
            .unwrap()
            .contains("confirmed-native-state")
    );
    let writes = state["writes"].clone();
    refused(&fixture.run("apply", &plan), "concord.project.stale");
    let recovery = decoded(&fixture.run("recover", &plan));
    assert_eq!(recovery["project"]["desired_state"], "observed");
    assert_eq!(fixture.state()["writes"], writes);
}

#[test]
fn membership() {
    let fixture = Fixture::new("ordinary");
    let action = intent(json!({"kind":"candidates","issues":["I_one","I_two"],"present":true}));
    let plan = decoded(&fixture.run("prepare", &action));
    let result = decoded(&fixture.run("apply", &plan));
    assert_eq!(result["project"]["completed"].as_array().unwrap().len(), 2);
    let again = decoded(&fixture.run("prepare", &action));
    assert_eq!(
        decoded(&fixture.run("apply", &again))["project"]["outcome"],
        "unchanged"
    );
    let remove = intent(json!({"kind":"candidates","issues":["I_one"],"present":false}));
    let plan = decoded(&fixture.run("prepare", &remove));
    decoded(&fixture.run("apply", &plan));
    let state = fixture.state();
    assert_eq!(state["items"].as_array().unwrap().len(), 1);
    assert_eq!(state["issues"][0]["state"], "OPEN");
}

#[test]
fn partial() {
    for mode in ["partial", "changed-type", "context", "request-context"] {
        let fixture = Fixture::new(mode);
        let plan = decoded(&fixture.run(
            "prepare",
            &intent(json!({"kind":"candidates","issues":["I_one","I_two"],"present":true})),
        ));
        let code = match mode {
            "partial" => "concord.project.indeterminate",
            "context" => "concord.project.context.changed",
            _ => "concord.project.interrupted",
        };
        let error = refused(&fixture.run("apply", &plan), code);
        let completed = if mode == "request-context" { 0 } else { 1 };
        assert_eq!(
            error["error"]["details"]["completed"]
                .as_array()
                .unwrap()
                .len(),
            completed
        );
        assert_eq!(
            fixture.state()["items"].as_array().unwrap().len(),
            completed
        );
        let writes = fixture.state()["writes"].clone();
        let result = decoded(&fixture.run("recover", &plan));
        assert_eq!(result["project"]["desired_state"], "unconfirmed");
        assert_eq!(fixture.state()["writes"], writes);
        assert_eq!(
            result["project"]["planning_records"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }
}

#[test]
fn uncertain() {
    for mode in ["lost", "record-failure"] {
        let fixture = Fixture::new(mode);
        let plan = decoded(&fixture.run(
            "prepare",
            &intent(json!({"kind":"candidates","issues":["I_one"],"present":true})),
        ));
        let code = if mode == "lost" {
            "concord.project.indeterminate"
        } else {
            "concord.project.record.indeterminate"
        };
        refused(&fixture.run("apply", &plan), code);
        let writes = fixture.state()["writes"].clone();
        let recovered = decoded(&fixture.run("recover", &plan));
        assert_eq!(recovered["project"]["desired_state"], "observed");
        assert_eq!(fixture.state()["writes"], writes);
    }
}

#[test]
fn qualification() {
    for mode in ["auto", "unknown", "foreign", "workflow", "stale"] {
        let fixture = Fixture::new("ordinary");
        let action = intent(json!({"kind":"candidates","issues":["I_one"],"present":true}));
        if mode == "stale" {
            let plan = decoded(&fixture.run("prepare", &action));
            fixture.change(|state| state["project"]["title"] = "Human edit".into());
            refused(&fixture.run("apply", &plan), "concord.project.stale");
        } else {
            fixture.change(|state| match mode {
                "auto" => state["issues"][0]["issueType"]["name"] = "Auto".into(),
                "unknown" => state["issues"][0]["issueType"] = serde_json::Value::Null,
                "foreign" => state["issues"][0]["repository"]["owner"]["id"] = "O_foreign".into(),
                "workflow" => state["workflows"] = workflow(),
                _ => unreachable!(),
            });
            let code = match mode {
                "foreign" => "identity",
                "workflow" => "automation",
                _ => "custody",
            };
            refused(
                &fixture.run("prepare", &action),
                &format!("concord.project.{code}"),
            );
        }
        assert!(fixture.state()["writes"].as_array().unwrap().is_empty());
    }
}

#[test]
fn fields() {
    let fixture = Fixture::new("ordinary");
    refused(
        &fixture.run(
            "prepare",
            &intent(json!({"kind":"rename","field":"F_issue","name":"Renamed"})),
        ),
        "concord.project.ownership",
    );
    refused(
        &fixture.run(
            "prepare",
            &intent(json!({"kind":"rename","field":"F_local","name":"Planning","options":[]})),
        ),
        "concord.project.identity",
    );
    let plan = decoded(&fixture.run(
        "prepare",
        &intent(json!({"kind":"detach","field":"F_issue","origin":"IF_priority"})),
    ));
    decoded(&fixture.run("apply", &plan));
    let options = json!([{"name":"High","color":"RED","description":""}]);
    let plan = decoded(&fixture.run(
        "prepare",
        &intent(
            json!({"kind":"field","name":"Priority","datatype":"SINGLE_SELECT","options":options}),
        ),
    ));
    let value = decoded(&fixture.run("apply", &plan));
    assert_eq!(value["project"]["result"]["authority"], "project");
    assert_eq!(value["project"]["result"]["options"][0]["id"], "choice-0");
}

#[test]
fn views() {
    let fixture = Fixture::new("ordinary");
    let action = intent(
        json!({"kind":"view","name":"Review","layout":"table","filter":"is:issue","visible":["F_local"],"group":["F_local"],"vertical":[],"sort":[{"field":"F_local","direction":"asc"}]}),
    );
    let plan = decoded(&fixture.run("prepare", &action));
    let value = decoded(&fixture.run("apply", &plan));
    let id = value["project"]["result"]["id"].clone();
    let plan = decoded(&fixture.run(
        "prepare",
        &intent(json!({"kind":"revise","view":id,"layout":"board","filter":"is:issue is:open"})),
    ));
    let value = decoded(&fixture.run("apply", &plan));
    assert_eq!(value["project"]["result"]["id"], id);
    assert_eq!(value["project"]["result"]["filter"], "is:issue is:open");
    let mut unsupported = action;
    unsupported["action"]["name"] = "Other".into();
    unsupported["action"]["visible"] = json!(["F_type"]);
    refused(
        &fixture.run("prepare", &unsupported),
        "concord.project.unsupported",
    );
}

#[test]
fn establishment() {
    let fixture = Fixture::new("ordinary");
    let visible = decoded(&fixture.target("list", "PerishLab", &json!({})));
    assert_eq!(
        visible["project"]["inventory"]["projects"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let plan = decoded(&fixture.target(
        "prepare",
        "PerishLab",
        &intent(json!({"kind":"create","title":"New planning scope"})),
    ));
    let result = decoded(&fixture.run("apply", &plan));
    assert_eq!(result["project"]["project"]["public"], false);
    assert_eq!(result["project"]["project"]["id"], "PVT_new");
    let workflow = intent(json!({"kind":"workflow","workflow":"WF_default"}));
    let plan = decoded(&fixture.target("prepare", "PerishLab/2", &workflow));
    decoded(&fixture.run("apply", &plan));
    assert!(fixture.state()["workflows"].as_array().unwrap().is_empty());
}

fn workflow() -> serde_json::Value {
    json!([{"id":"WF_auto","name":"Automatic closure","enabled":true,"project":{"id":"PVT_plan"}}])
}

#[test]
fn preservation() {
    let fixture = Fixture::new("ordinary");
    fixture.change(|state| {
        let content=state["issues"][0].clone();
        let value=json!({"__typename":"ProjectV2ItemFieldSingleSelectValue","field":{"id":"F_local"}});
        let values=super::planning::connection(vec![value],false);
        let item=json!({"id":"ITEM_one","type":"ISSUE","isArchived":false,"project":{"id":"PVT_plan"},"content":content,"fieldValues":values});
        state["items"]=json!([item]);
    });
    refused(
        &fixture.run(
            "prepare",
            &intent(json!({"kind":"remove","field":"F_local"})),
        ),
        "concord.project.populated",
    );
    let option = json!({"id":"old","name":"Reviewed","color":"BLUE","description":""});
    let plan = decoded(&fixture.run(
        "prepare",
        &intent(json!({"kind":"rename","field":"F_local","name":"Planning","options":[option]})),
    ));
    let value = decoded(&fixture.run("apply", &plan));
    assert_eq!(value["project"]["result"]["options"][0]["id"], "old");
    assert_eq!(
        fixture.state()["items"][0]["fieldValues"]["nodes"][0]["field"]["id"],
        "F_local"
    );
}

#[test]
fn linkage() {
    let fixture = Fixture::new("ordinary");
    let plan = decoded(&fixture.run(
        "prepare",
        &intent(json!({"kind":"link","repository":"R_repo","present":true})),
    ));
    decoded(&fixture.run("apply", &plan));
    assert_eq!(fixture.state()["repositories"][0]["id"], "R_repo");
    let plan = decoded(&fixture.run(
        "prepare",
        &intent(json!({"kind":"link","repository":"R_repo","present":false})),
    ));
    decoded(&fixture.run("apply", &plan));
    assert!(
        fixture.state()["repositories"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}
