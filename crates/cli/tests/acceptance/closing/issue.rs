use super::super::world::{decoded, refused};
use super::fixture::{World, output};
use serde_json::Value;

#[test]
fn completed() {
    let world = World::new();
    let plan = world.prepare();
    let result = decoded(&world.run("apply", Some(&plan)));
    assert_eq!(result["closure"]["outcome"], "confirmed-closed");
    assert_eq!(result["closure"]["attempted"], true);
    assert_eq!(result["closure"]["current"]["issue"]["state"], "closed");
    assert_eq!(result["closure"]["current"]["issue"]["reason"], "completed");
    assert_eq!(
        result["closure"]["current"]["acceptance"]["verification"],
        "manual"
    );
    assert_eq!(world.state()["attempts"], 1);
    let recovered = decoded(&world.run("recover", Some(&plan)));
    assert_eq!(recovered["closure"]["outcome"], "observed-closed");
    assert_eq!(recovered["closure"]["attempted"], false);
    let repeated = decoded(&world.run("apply", Some(&plan)));
    assert_eq!(repeated["closure"]["outcome"], "observed-closed");
    assert_eq!(repeated["closure"]["attempted"], false);
    assert_eq!(world.state()["attempts"], 1);
    let human = decoded(&output(world.command("recover"), Some(&plan)));
    assert_eq!(human["closure"], recovered["closure"]);
}

#[test]
fn drift() {
    for (mode, code) in [
        ("stamp", "stale"),
        ("closure", "stale"),
        ("body", "acceptance"),
        ("needs", "needs"),
        ("child", "acceptance"),
        ("blocked", "acceptance"),
        ("reopen", "acceptance"),
        ("node", "changed"),
        ("repository", "changed"),
        ("coordinate", "identity"),
    ] {
        let world = World::new();
        let plan = world.prepare();
        world.mode(mode);
        let result = world.run("apply", Some(&plan));
        refused(&result, &format!("concord.issue.close.{code}"));
        let error: Value = serde_json::from_slice(&result.stderr).unwrap();
        assert_eq!(error["error"]["details"]["outcome"], "not-attempted");
        assert_eq!(error["error"]["details"]["attempted"], false);
        assert_eq!(world.state()["attempts"], 0, "{mode}");
    }
    let world = World::new();
    let plan = world.prepare();
    world.mode("declaration");
    refused(&world.run("apply", Some(&plan)), "acceptance");
    assert_eq!(world.state()["attempts"], 0);
}

#[test]
fn unknown() {
    for mode in ["lost", "ack", "readback", "fail-before", "forbidden"] {
        let world = World::new();
        let plan = world.prepare();
        world.mode(mode);
        let result = world.run("apply", Some(&plan));
        refused(&result, "concord.issue.close.indeterminate");
        let error: Value = serde_json::from_slice(&result.stderr).unwrap();
        assert_eq!(
            error["error"]["cookbook"],
            "concord.issue.close.indeterminate"
        );
        assert_eq!(error["error"]["details"]["attempted"], true);
        assert_eq!(world.state()["attempts"], 1, "{mode}");
        world.mode("ok");
        let recovered = decoded(&world.run("recover", Some(&plan)));
        let expected = if mode == "fail-before" || mode == "forbidden" {
            "observed-open"
        } else {
            "observed-closed"
        };
        assert_eq!(recovered["closure"]["outcome"], expected, "{mode}");
        assert_eq!(recovered["closure"]["attempted"], false);
        assert_eq!(world.state()["attempts"], 1);
    }
}

#[test]
fn race() {
    let world = World::new();
    let plan = world.prepare();
    world.mode("body-after");
    refused(
        &world.run("apply", Some(&plan)),
        "concord.issue.close.indeterminate",
    );
    let state = world.state();
    assert_eq!(state["closed"], true);
    assert_eq!(state["attempts"], 1);
    let recovered = decoded(&world.run("recover", Some(&plan)));
    assert_eq!(recovered["closure"]["outcome"], "unconfirmed");
    assert_eq!(recovered["closure"]["basis_matches"], false);
    assert_eq!(
        recovered["closure"]["current"]["acceptance"]["verdict"],
        "unmet"
    );
    assert_eq!(world.state()["attempts"], 1);
}

#[test]
fn basis() {
    for (mode, code) in [
        ("auto", "type"),
        ("unknown", "type"),
        ("permission", "precondition"),
        ("head-drift", "changed"),
        ("reason", "schema"),
        ("legacy", "acceptance"),
        ("partial", "provider"),
    ] {
        let world = World::new();
        world.mode(mode);
        refused(
            &world.run("prepare", None),
            &format!("concord.issue.close.{code}"),
        );
        assert_eq!(world.state()["attempts"], 0);
    }
    let world = World::new();
    world.release();
    refused(
        &world.run("prepare", None),
        "concord.issue.close.acceptance",
    );
    assert_eq!(world.state()["attempts"], 0);
}

#[test]
fn explicit() {
    let world = World::new();
    let plan = world.prepare();
    let mut command = world.command("apply");
    command.arg("--json");
    refused(&output(command, Some(&plan)), "concord.apply.required");
    assert_eq!(world.state()["attempts"], 0);
    let mut foreign = plan;
    foreign["plan"]["issue"]["number"] = 84.into();
    refused(
        &world.run("apply", Some(&foreign)),
        "concord.issue.close.plan",
    );
    assert_eq!(world.state()["attempts"], 0);
}

#[test]
fn grammar() {
    let scratch = tempfile::tempdir().unwrap();
    let output = crate::spawn::concord(scratch.path())
        .args(["issue", "close", "--help"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let help = String::from_utf8_lossy(&output.stdout);
    assert!(help.contains("no atomic body/review"));
    assert!(help.contains("recover"));
    assert!(help.contains("No local estate"));
}
