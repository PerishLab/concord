use super::super::world::{decoded, refused};
use super::fixture::{World, output};
use serde_json::{Value, json};

fn error(result: &std::process::Output) -> Value {
    serde_json::from_slice::<Value>(&result.stderr).unwrap()["error"].clone()
}

fn check(diagnostic: &Value, name: &str, verdict: &str) {
    assert_eq!(diagnostic["verdict"], verdict);
    assert_eq!(diagnostic["truncated"], false);
    let checks = diagnostic["checks"].as_array().unwrap();
    assert_eq!(checks.len(), 1);
    assert_eq!(checks[0]["name"], name);
    assert_eq!(checks[0]["verdict"], verdict);
    assert!(!checks[0]["reason"].as_str().unwrap().is_empty());
}

#[test]
fn preparation() {
    for (mode, name, verdict) in [
        ("child", "children", "unmet"),
        ("unordered", "freshness", "unknown"),
    ] {
        let world = World::new();
        world.mode(mode);
        let result = world.run("prepare", None);
        refused(&result, "concord.issue.close.acceptance");
        let failure = error(&result);
        check(&failure["details"], name, verdict);
        assert!(failure["message"].as_str().unwrap().contains(name));
        assert!(failure["message"].as_str().unwrap().contains(verdict));
        assert_eq!(world.state()["attempts"], 0);
        assert_eq!(world.state()["calls"], 11);
        let mut command = world.command("prepare");
        command.arg("--observe-timeout").arg("10");
        let human = output(command, None);
        assert!(!human.status.success());
        let text = String::from_utf8_lossy(&human.stderr);
        assert!(text.contains(name) && text.contains(verdict));
        assert_eq!(world.state()["calls"], 22);
    }
}

#[test]
fn before() {
    for (mode, name, verdict) in [
        ("child", "children", "unmet"),
        ("unordered", "freshness", "unknown"),
    ] {
        let world = World::new();
        let plan = world.prepare();
        let calls = world.state()["calls"].as_u64().unwrap();
        world.mode(mode);
        let result = world.run("apply", Some(&plan));
        refused(&result, "concord.issue.close.acceptance");
        let failure = error(&result);
        assert_eq!(failure["details"]["attempted"], false);
        assert_eq!(failure["details"]["outcome"], "not-attempted");
        check(&failure["details"]["cause"], name, verdict);
        assert_eq!(world.state()["attempts"], 0);
        assert_eq!(world.state()["calls"].as_u64().unwrap() - calls, 11);
    }
}

#[test]
fn confirmation() {
    for (mode, name, verdict) in [
        ("after-unmet", "blockers", "unmet"),
        ("after-unknown", "freshness", "unknown"),
    ] {
        let world = World::new();
        let plan = world.prepare();
        let calls = world.state()["calls"].as_u64().unwrap();
        world.mode(mode);
        let result = world.run("apply", Some(&plan));
        refused(&result, "concord.issue.close.indeterminate");
        let failure = error(&result);
        let details = &failure["details"];
        assert_eq!(details["attempted"], true);
        assert_eq!(details["outcome"], "unknown");
        assert_eq!(details["stage"], "acceptance");
        assert_eq!(details["cause_code"], "concord.issue.close.acceptance");
        check(&details["cause"], name, verdict);
        let message = failure["message"].as_str().unwrap();
        assert!(message.contains("recover") && message.contains(name) && message.contains(verdict));
        assert_eq!(world.state()["calls"].as_u64().unwrap() - calls, 23);
        assert_eq!(world.state()["attempts"], 1);
        world.mode("ok");
        let recovered = decoded(&world.run("recover", Some(&plan)));
        assert_eq!(recovered["closure"]["outcome"], "observed-closed");
        assert_eq!(recovered["closure"]["attempted"], false);
        assert_eq!(
            recovered["closure"]["current"]["acceptance"]["verdict"],
            "satisfied"
        );
        check(&failure["details"]["cause"], name, verdict);
        assert_eq!(world.state()["calls"].as_u64().unwrap() - calls, 34);
        assert_eq!(world.state()["attempts"], 1);
        assert_eq!(failure["cookbook"], "concord.issue.close.indeterminate");
        assert_ne!(failure["details"]["cause"], json!(null));
    }
}
