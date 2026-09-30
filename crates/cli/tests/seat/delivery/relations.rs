use super::provider::consume;
use super::unix::prepare;
use serde_json::{Value, json};
use std::path::Path;

pub fn verify(space: &Path, provider: &str) {
    let file = space.join("relationships");
    let first = relation(2);
    let second = relation(3);
    write(&file, &json!([second, first]));
    let plan = prepare(space, provider, "2", true)
        .output()
        .expect("prepare");
    assert!(
        plan.status.success(),
        "{}",
        String::from_utf8_lossy(&plan.stderr)
    );
    let value: Value = serde_json::from_slice(&plan.stdout).expect("plan");
    assert_eq!(
        value["plan"]["delivery"]["issue"]["blocking"][0]["number"],
        2
    );
    write(&file, &json!([first, second]));
    std::fs::write(space.join("fail-create"), "once").expect("stop after revalidation");
    refuses(
        &consume(space, provider, &plan.stdout),
        "concord.delivery.provider",
    );
    let mut opened = second.clone();
    opened["state"] = json!("OPEN");
    for changed in [
        json!([first]),
        json!([first, second, relation(4)]),
        json!([first, opened]),
    ] {
        write(&file, &changed);
        refuses(
            &consume(space, provider, &plan.stdout),
            "concord.delivery.stale",
        );
    }
    std::fs::remove_file(file).expect("restore empty relationships");
}

fn relation(number: u64) -> Value {
    json!({
        "id": format!("I_dependency_{number}"), "number": number,
        "url": format!("https://github.com/PerishLab/probe/issues/{number}"),
        "title": format!("dependency {number}"), "state": "CLOSED",
        "repository": {"nameWithOwner": "PerishLab/probe"},
    })
}

fn write(path: &Path, value: &Value) {
    std::fs::write(path, serde_json::to_vec(value).expect("relationships"))
        .expect("provider facts");
}

fn refuses(output: &std::process::Output, code: &str) {
    assert!(!output.status.success());
    let error: Value = serde_json::from_slice(&output.stderr).expect("refusal");
    assert_eq!(error["error"]["code"], code);
}
