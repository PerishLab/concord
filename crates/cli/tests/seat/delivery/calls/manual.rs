use super::super::super::execution::unix::{git, text};
use super::super::fixture::Native;
use super::super::unix::{refused, success};
use serde_json::{Value, json};
use std::process::Output;

#[path = "world.rs"]
mod world;

fn review(seat: &Native<'_>) -> Value {
    json!({
        "schema": "concord.manual-review/v1",
        "verification": "manual",
        "repository": "R_kgDOUsHIOQ",
        "issue": "I_delivery",
        "source": text(&seat.member, &["rev-parse", "HEAD"]),
        "target": text(&seat.source, &["rev-parse", "HEAD"]),
        "tree": text(&seat.member, &["rev-parse", "HEAD^{tree}"]),
        "reason": "Reviewed the exact documentation delta and manual governance exception.",
    })
}

fn prepare(seat: &Native<'_>, mode: &str, review: &Value) -> Output {
    let file = seat.space.join("review.json");
    std::fs::write(&file, review.to_string()).expect("review");
    let seen = success(seat.space, &["issue", "show", &seat.issue]);
    seat.command("prepare", mode)
        .args([
            "--revision",
            &seen["anchor"]["revision"].to_string(),
            "--review",
            file.to_str().expect("review"),
        ])
        .output()
        .expect("prepare")
}

#[test]
fn exact() {
    let temp = tempfile::tempdir().expect("fixture");
    let seat = Native::governance(temp.path(), "R_kgDOUsHIOQ");
    refused(&seat.prepare("plumb"), "concord.delivery.authority");
    refused(&seat.prepare("wharf-native"), "concord.delivery.authority");
    refused(
        &seat.prepare("governance-manual"),
        "concord.delivery.review",
    );
    let input = review(&seat);
    refused(&prepare(&seat, "plumb", &input), "concord.delivery.review");
    let prepared = prepare(&seat, "governance-manual", &input);
    assert!(
        prepared.status.success(),
        "{}",
        String::from_utf8_lossy(&prepared.stderr)
    );
    let plan: Value = serde_json::from_slice(&prepared.stdout).expect("plan");
    assert_eq!(plan["plan"]["authority"]["kind"], "governance-manual");
    assert_eq!(plan["plan"]["authority"]["review"], input);
    let body = plan["plan"]["delivery"]["pull"]["body"]
        .as_str()
        .expect("body");
    assert!(body.contains("Verification: manual review"));
    assert!(body.contains("No automated gate checks are claimed"));
    assert!(body.contains("concord.manual-review/v1"));
    let landed = seat.land("governance-manual", &prepared.stdout);
    assert!(
        landed.status.success(),
        "{}",
        String::from_utf8_lossy(&landed.stderr)
    );
    let result: Value = serde_json::from_slice(&landed.stdout).expect("landed");
    assert_eq!(result["merged"], true);
    assert_eq!(result["released"], true);
    assert!(!seat.member.exists());
    let message = text(&seat.source, &["log", "-1", "--format=%B"]);
    assert!(message.contains("Native-Gate-Proof:"));
    assert!(!message.contains("Plumb-Guard-Proof:"));
    assert!(
        std::fs::read_to_string(seat.space.join("status-call"))
            .expect("status")
            .contains("manual / governance")
    );
    let audit = success(seat.space, &["audit"]);
    assert!(
        audit["agreement"]["faults"]
            .as_array()
            .expect("faults")
            .is_empty()
    );
    assert!(
        seat.land("governance-manual", &prepared.stdout)
            .status
            .success()
    );
    assert!(!seat.space.join("gate-runs").exists());
}

#[test]
fn invalid() {
    let temp = tempfile::tempdir().expect("fixture");
    let seat = Native::governance(temp.path(), "R_kgDOUsHIOQ");
    for field in [
        "schema",
        "verification",
        "repository",
        "issue",
        "source",
        "target",
        "tree",
        "reason",
    ] {
        let mut input = review(&seat);
        input[field] = json!("");
        refused(
            &prepare(&seat, "governance-manual", &input),
            "concord.delivery.review",
        );
    }
    let mut input = review(&seat);
    input["reason"] = json!(" ".repeat(4097));
    refused(
        &prepare(&seat, "governance-manual", &input),
        "concord.delivery.review",
    );
    assert!(!seat.space.join("pull-state").exists());
}

#[test]
fn tampered() {
    let temp = tempfile::tempdir().expect("fixture");
    let seat = Native::governance(temp.path(), "R_kgDOUsHIOQ");
    let prepared = prepare(&seat, "governance-manual", &review(&seat));
    assert!(
        prepared.status.success(),
        "{}",
        String::from_utf8_lossy(&prepared.stderr)
    );
    let plan: Value = serde_json::from_slice(&prepared.stdout).expect("plan");
    let mut changed = plan.clone();
    changed["plan"]["authority"]["review"]["reason"] = json!("Different review");
    refused(
        &seat.land("governance-manual", &serde_json::to_vec(&changed).unwrap()),
        "concord.delivery.stale",
    );
    changed = plan.clone();
    changed["plan"]["authority"]["review"]["source"] = json!("0".repeat(40));
    refused(
        &seat.land("governance-manual", &serde_json::to_vec(&changed).unwrap()),
        "concord.delivery.review",
    );
    changed = plan;
    changed["plan"]["authority"]["evidence"]["authority"] = json!("another implementation");
    refused(
        &seat.land("governance-manual", &serde_json::to_vec(&changed).unwrap()),
        "concord.delivery.authority",
    );
    assert!(!seat.space.join("pull-state").exists());
}

#[test]
fn identity() {
    let temp = tempfile::tempdir().expect("fixture");
    let seat = Native::governance(temp.path(), "R_other");
    refused(
        &prepare(&seat, "governance-manual", &review(&seat)),
        "concord.delivery.authority",
    );
    let temp = tempfile::tempdir().expect("fixture");
    let seat = Native::open(temp.path(), "R_kgDOUesM0Q");
    refused(
        &prepare(&seat, "governance-manual", &review(&seat)),
        "concord.delivery.authority",
    );
}

fn change(seat: &Native<'_>, file: &str, content: Option<&str>) {
    let seen = success(seat.space, &["issue", "show", &seat.issue]);
    let revision = seen["anchor"]["revision"].as_i64().expect("revision");
    success(
        seat.space,
        &[
            "member",
            "claim",
            &seat.issue,
            "--claim",
            file,
            "--revision",
            &revision.to_string(),
        ],
    );
    match content {
        Some(content) => std::fs::write(seat.member.join(file), content).expect("file"),
        None => std::fs::remove_file(seat.member.join(file)).expect("remove"),
    }
    git(&seat.member, &["add", file]);
    git(&seat.member, &["commit", "-m", "changed input"]);
    success(
        seat.space,
        &[
            "member",
            "prove",
            &seat.issue,
            "--revision",
            &(revision + 1).to_string(),
        ],
    );
}

#[test]
fn policy() {
    let temp = tempfile::tempdir().expect("fixture");
    let seat = Native::governance(temp.path(), "R_kgDOUsHIOQ");
    change(&seat, "AGENTS.md", Some("No manual exception.\n"));
    refused(
        &prepare(&seat, "governance-manual", &review(&seat)),
        "concord.delivery.review",
    );
}

#[test]
fn governed() {
    let temp = tempfile::tempdir().expect("fixture");
    let seat = Native::governance(temp.path(), "R_kgDOUsHIOQ");
    change(&seat, "plumb.toml", Some("[identity]\nns = \"fixture\"\n"));
    refused(
        &prepare(&seat, "governance-manual", &review(&seat)),
        "concord.delivery.native",
    );
}

#[test]
fn base() {
    let temp = tempfile::tempdir().expect("fixture");
    let seat = Native::governance(temp.path(), "R_kgDOUsHIOQ");
    std::fs::write(
        seat.source.join("plumb.toml"),
        "[identity]\nns = \"fixture\"\n",
    )
    .expect("governed base");
    git(&seat.source, &["add", "plumb.toml"]);
    git(&seat.source, &["commit", "-m", "govern base"]);
    git(&seat.source, &["push", "origin", "main"]);
    git(&seat.member, &["rebase", "main"]);
    change(&seat, "plumb.toml", None);
    refused(
        &prepare(&seat, "governance-manual", &review(&seat)),
        "concord.delivery.native",
    );
}

#[test]
fn lost() {
    let temp = tempfile::tempdir().expect("fixture");
    let seat = Native::governance(temp.path(), "R_kgDOUsHIOQ");
    let prepared = prepare(&seat, "governance-manual", &review(&seat));
    assert!(prepared.status.success());
    std::fs::write(seat.space.join("lose-proof"), "one").expect("flag");
    refused(
        &seat.land("governance-manual", &prepared.stdout),
        "concord.delivery.landed",
    );
    assert!(seat.member.exists());
}
