mod preflight;
mod world;
use serde_json::{Value, json};
use world::{World, decoded, intent, refused};

#[test]
fn lifecycle() {
    let world = World::new();
    let before = world.state()["issue"].clone();
    for value in [json!("Verify"), Value::Null, json!("Settle")] {
        let plan = decoded(&world.phase("prepare", &intent(value.clone())));
        let after = decoded(&world.phase("apply", &plan));
        assert_eq!(after["phase"]["outcome"], "confirmed-native-state");
        let issue = &after["phase"]["current"]["issue"];
        for key in [
            "body",
            "labels",
            "blockedBy",
            "blocking",
            "comments",
            "state",
            "updatedAt",
        ] {
            assert_eq!(issue[key], before[key], "{key}");
        }
        assert_eq!(
            issue["issueFieldValues"]["nodes"][0],
            before["issueFieldValues"]["nodes"][0]
        );
        assert_eq!(
            decoded(&world.phase("recover", &plan))["phase"]["desired_state"],
            "observed"
        );
    }
    assert_eq!(world.writes(), 3);
    let state = world.state();
    let fields = &state["writes"][1]["variables"]["input"]["issueFields"];
    assert_eq!(fields.as_array().unwrap().len(), 1);
    assert_eq!(fields[0]["delete"], true);
    assert!(fields[0].get("singleSelectOptionId").is_none());
}

#[test]
fn stale() {
    let world = World::new();
    let plan = decoded(&world.phase("prepare", &intent(json!("Verify"))));
    let other = decoded(&world.phase("prepare", &intent(json!("Deliver"))));
    decoded(&world.phase("apply", &other));
    refused(&world.phase("apply", &plan), "concord.phase.stale");
    assert_eq!(world.writes(), 1);
    assert_eq!(world.state()["issue"]["updatedAt"], "original");
    assert_eq!(
        decoded(&world.phase("recover", &plan))["phase"]["desired_state"],
        "unconfirmed"
    );
    assert_eq!(world.writes(), 1);
}

#[test]
fn unknown() {
    let world = World::new();
    let plan = decoded(&world.phase("prepare", &intent(json!("Verify"))));
    world.change(|state| state["mode"] = "unknown".into());
    refused(&world.phase("apply", &plan), "concord.phase.indeterminate");
    assert_eq!(
        decoded(&world.phase("recover", &plan))["phase"]["desired_state"],
        "observed"
    );
    assert_eq!(world.writes(), 1);
}

#[test]
fn observations() {
    let world = World::new();
    for mode in ["brief", "preflight", "next"] {
        let key = if mode == "next" { "next" } else { mode };
        assert_eq!(world.observe(mode)[key]["phase"]["status"], "unset");
    }
    let plan = decoded(&world.phase("prepare", &intent(json!("Verify"))));
    decoded(&world.phase("apply", &plan));
    for mode in ["brief", "preflight", "next"] {
        let key = if mode == "next" { "next" } else { mode };
        let value = world.observe(mode);
        let phase = &value[key]["phase"];
        assert_eq!(phase["value"], "Verify");
        assert_eq!(phase["option"], "IFSSO_v");
        assert_eq!(phase["authority"], "issue");
        assert_eq!(phase["binding"]["organization"], "O_lab");
    }
    assert_eq!(world.observe("next")["next"]["item"], "ITEM_first");
    world.unbind();
    assert_eq!(
        world.observe("brief")["brief"]["phase"]["status"],
        "not-configured"
    );
}

#[test]
fn schema() {
    for mode in [
        "owner",
        "field",
        "option",
        "type",
        "visibility",
        "duplicate",
        "truncated",
        "unavailable",
    ] {
        let world = World::new();
        world.change(|state| match mode {
            "owner" => state["owner"] = "O_foreign".into(),
            "field" => state["field"]["id"] = "PVTSSF_same_name".into(),
            "option" => state["field"]["options"][2]["id"] = "replaced".into(),
            "type" => state["field"]["__typename"] = "ProjectV2SingleSelectField".into(),
            "visibility" => state["field"]["visibility"] = "PUBLIC".into(),
            "duplicate" => state["field"]["options"][2] = state["field"]["options"][0].clone(),
            "truncated" => {
                state["issue"]["issueFieldValues"]["pageInfo"]["hasNextPage"] = true.into()
            }
            "unavailable" => state["mode"] = "unavailable".into(),
            _ => unreachable!(),
        });
        if mode != "unavailable" {
            let status = if mode == "truncated" {
                "unavailable"
            } else {
                "invalid"
            };
            assert_eq!(
                world.observe("brief")["brief"]["phase"]["status"],
                status,
                "{mode}"
            );
        }
        let code = if matches!(mode, "truncated" | "unavailable") {
            "concord.phase.unavailable"
        } else {
            "concord.phase.invalid"
        };
        refused(&world.phase("prepare", &intent(json!("Verify"))), code);
        assert_eq!(world.writes(), 0);
    }
}

#[test]
fn custody() {
    for kind in ["Auto", "Unknown", ""] {
        let world = World::new();
        world.change(|state| state["issue"]["issueType"]["name"] = kind.into());
        refused(
            &world.phase("prepare", &intent(json!("Verify"))),
            "concord.phase.custody",
        );
        assert_eq!(world.writes(), 0);
    }
    let world = World::new();
    refused(
        &world.phase("prepare", &json!({"reason":"No implicit clear"})),
        "concord.input.json",
    );
    refused(
        &world.phase("prepare", &json!({"reason":" ","value":"Verify"})),
        "concord.phase.reason",
    );
    refused(
        &world.phase("prepare", &intent(json!("Done"))),
        "concord.phase.intent",
    );
}

#[test]
fn readback() {
    for mode in ["stamp", "foreign"] {
        let world = World::new();
        world.change(|state| state["mode"] = mode.into());
        let plan = decoded(&world.phase("prepare", &intent(json!("Verify"))));
        let result = world.phase("apply", &plan);
        if mode == "foreign" {
            refused(&result, "concord.phase.indeterminate");
        } else {
            assert_eq!(
                decoded(&result)["phase"]["acceptance"]["after_updated"],
                "phase-only"
            );
        }
        decoded(&world.phase("recover", &plan));
        assert_eq!(world.writes(), 1);
    }
}

#[test]
fn presentation() {
    let world = World::new();
    let plan = decoded(&world.phase("prepare", &intent(json!("Verify"))));
    decoded(&world.phase("apply", &plan));
    for args in [
        vec!["issue", "brief", "PerishLab/concord#4"],
        vec!["issue", "preflight", "existing", "PerishLab/concord#4"],
        vec!["project", "next", "PerishLab/1"],
    ] {
        let human = world.human(&args);
        for part in ["Verify", "IFSS_phase", "IFSSO_v", "O_lab"] {
            assert!(human.contains(part), "{human}");
        }
    }
    world.change(|state| state["field"]["options"][2]["id"] = "replaced".into());
    let next = world.observe("next");
    assert_eq!(next["next"]["item"], "ITEM_first");
    assert_eq!(next["next"]["phase"]["status"], "invalid");
}

#[test]
fn opaque() {
    let world = World::new();
    world.change(|state| {
        state["issue"]["issueFieldValues"]["nodes"][0] =
            json!({"__typename":"UnknownValue","id":"V_other"})
    });
    assert_eq!(
        world.observe("brief")["brief"]["phase"]["status"],
        "unavailable"
    );
    refused(
        &world.phase("prepare", &intent(Value::Null)),
        "concord.phase.unavailable",
    );
    assert_eq!(world.writes(), 0);
}

#[test]
fn minimal() {
    for expected in [Value::Null, json!("Verify")] {
        let world = World::new();
        if !expected.is_null() {
            let plan = decoded(&world.phase("prepare", &intent(expected.clone())));
            decoded(&world.phase("apply", &plan));
        }
        let writes = world.writes();
        world.change(|state| state["mode"] = "minimal".into());
        for mode in ["brief", "preflight", "next"] {
            let key = if mode == "next" { "next" } else { mode };
            let result = world.observe(mode);
            let phase = &result[key]["phase"];
            assert_eq!(
                phase["status"],
                if expected.is_null() { "unset" } else { "set" }
            );
            assert_eq!(phase["value"], expected);
        }
        assert_eq!(world.writes(), writes);
    }
}

#[test]
fn separation() {
    for key in ["comments", "subIssues", "blocking"] {
        let world = World::new();
        let plan = decoded(&world.phase("prepare", &intent(json!("Verify"))));
        world.change(|state| state["issue"][key]["pageInfo"]["hasNextPage"] = true.into());
        assert_eq!(
            world.observe("next")["next"]["phase"]["status"],
            "unset",
            "{key}"
        );
        refused(
            &world.phase("prepare", &intent(json!("Verify"))),
            "concord.phase.unavailable",
        );
        refused(&world.phase("apply", &plan), "concord.phase.unavailable");
        refused(&world.phase("recover", &plan), "concord.phase.unavailable");
        assert_eq!(world.writes(), 0);
    }
    let world = World::new();
    let plan = decoded(&world.phase("prepare", &intent(json!("Verify"))));
    world.change(|state| state["issue"]["body"] = "A fresh external edit".into());
    refused(&world.phase("apply", &plan), "concord.phase.stale");
    let recovered = decoded(&world.phase("recover", &plan));
    assert_eq!(
        recovered["phase"]["current"]["issue"]["body"],
        "A fresh external edit"
    );
    assert_eq!(world.writes(), 0);
}
